use std::{
    fs,
    mem::{size_of, size_of_val},
    os::windows::{
        ffi::OsStrExt,
        fs::MetadataExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::Path,
    ptr::{addr_of_mut, null_mut},
};
use windows_sys::{
    Win32::{
        Foundation::{ERROR_SUCCESS, GENERIC_ALL, GENERIC_WRITE, LocalFree},
        Security::{
            ACCESS_ALLOWED_ACE, ACE_HEADER, ACL,
            Authorization::{ConvertStringSidToSidW, GetNamedSecurityInfoW, SE_FILE_OBJECT},
            DACL_SECURITY_INFORMATION, EqualSid, GetAce, GetTokenInformation, INHERIT_ONLY_ACE,
            IsWellKnownSid, OWNER_SECURITY_INFORMATION, PSID, SECURITY_MAX_SID_SIZE, TOKEN_QUERY,
            TOKEN_USER, TokenUser, WinBuiltinAdministratorsSid, WinLocalSystemSid,
        },
        Storage::FileSystem::{
            DELETE, FILE_APPEND_DATA, FILE_ATTRIBUTE_REPARSE_POINT, FILE_DELETE_CHILD,
            FILE_WRITE_ATTRIBUTES, FILE_WRITE_DATA, FILE_WRITE_EA, WRITE_DAC, WRITE_OWNER,
        },
        System::{
            SystemServices::{ACCESS_ALLOWED_ACE_TYPE, ACCESS_DENIED_ACE_TYPE},
            Threading::{GetCurrentProcess, OpenProcessToken},
        },
    },
    core::w,
};

struct LocalAllocation(*mut std::ffi::c_void);

impl Drop for LocalAllocation {
    fn drop(&mut self) {
        // Both successful Win32 calls that construct this guard require LocalFree.
        unsafe {
            LocalFree(self.0);
        }
    }
}

#[repr(C)]
struct TokenUserBuffer {
    user: TOKEN_USER,
    // GetTokenInformation puts the variable-length SID after TOKEN_USER in the same buffer.
    tail: [u8; SECURITY_MAX_SID_SIZE as usize],
}

pub(super) fn protected(path: &Path, metadata: &fs::Metadata, depth: usize) -> bool {
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return false;
    }
    let wide: Vec<_> = path.as_os_str().encode_wide().chain([0]).collect();
    let mut owner = null_mut();
    let mut dacl = null_mut();
    let mut descriptor = null_mut();
    // owner and dacl are interior pointers into descriptor and stay valid until it is freed.
    let code = unsafe {
        GetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &raw mut owner,
            null_mut(),
            &raw mut dacl,
            null_mut(),
            &raw mut descriptor,
        )
    };
    if code != ERROR_SUCCESS {
        return false;
    }
    let _descriptor = LocalAllocation(descriptor);
    if owner.is_null() || dacl.is_null() {
        return false;
    }

    let mut raw = null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut raw) } == 0 {
        return false;
    }
    // OpenProcessToken returned an owned token handle.
    let token = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut identity = TokenUserBuffer {
        user: TOKEN_USER::default(),
        tail: [0; SECURITY_MAX_SID_SIZE as usize],
    };
    let mut length = 0;
    let size = size_of_val(&identity) as u32;
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            addr_of_mut!(identity).cast(),
            size,
            &raw mut length,
        )
    } == 0
    {
        return false;
    }
    let mut installer_sid = null_mut();
    // Windows Modules Installer owns protected system directories on some installations.
    if unsafe {
        ConvertStringSidToSidW(
            w!("S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464"),
            &raw mut installer_sid,
        )
    } == 0
    {
        return false;
    }
    let _installer_sid = LocalAllocation(installer_sid);
    let trusted = |sid| unsafe {
        EqualSid(sid, identity.user.User.Sid) != 0
            || EqualSid(sid, installer_sid) != 0
            || IsWellKnownSid(sid, WinLocalSystemSid) != 0
            || IsWellKnownSid(sid, WinBuiltinAdministratorsSid) != 0
    };
    let mut writes = GENERIC_WRITE
        | GENERIC_ALL
        | DELETE
        | WRITE_DAC
        | WRITE_OWNER
        | FILE_DELETE_CHILD
        | FILE_WRITE_ATTRIBUTES
        | FILE_WRITE_EA;
    // Ancestors such as the volume root can allow creating unrelated children.
    if depth <= 1 {
        writes |= FILE_WRITE_DATA | FILE_APPEND_DATA;
    }
    trusted(owner) && acl_is_protected(dacl, writes, trusted)
}

fn acl_is_protected(dacl: *mut ACL, writes: u32, trusted: impl Fn(PSID) -> bool) -> bool {
    // GetNamedSecurityInfoW supplied this ACL. GetAce indexes its entries while the
    // descriptor is alive. Inspect the ACE type before reading its type-specific layout.
    unsafe {
        (0..u32::from((*dacl).AceCount)).all(|index| {
            let mut ace = null_mut();
            if GetAce(dacl, index, &raw mut ace) == 0 {
                return false;
            }
            let header = &*ace.cast::<ACE_HEADER>();
            if u32::from(header.AceFlags) & INHERIT_ONLY_ACE != 0 {
                return true;
            }
            match u32::from(header.AceType) {
                ACCESS_DENIED_ACE_TYPE => true,
                ACCESS_ALLOWED_ACE_TYPE
                    if usize::from(header.AceSize) >= size_of::<ACCESS_ALLOWED_ACE>() =>
                {
                    let allow = ace.cast::<ACCESS_ALLOWED_ACE>();
                    (*allow).Mask & writes == 0 || trusted(addr_of_mut!((*allow).SidStart).cast())
                }
                _ => false,
            }
        })
    }
}
