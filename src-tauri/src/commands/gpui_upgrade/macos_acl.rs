//! Darwin evaluates extended ACLs in addition to the POSIX mode bits.
use std::{
    ffi::{CString, c_char, c_int, c_void},
    io,
    os::unix::ffi::OsStrExt,
    path::Path,
    ptr::null_mut,
};

// The constants and signatures are from Darwin's <sys/acl.h>.
const ACL_TYPE_EXTENDED: c_int = 0x100;
const ACL_FIRST_ENTRY: c_int = 0;
const ACL_NEXT_ENTRY: c_int = -1;
const ACL_EXTENDED_DENY: c_int = 2;
// read data, execute/search, read attributes, read extended attributes, read security, synchronize
const READ_ONLY: u64 = (1 << 1) | (1 << 3) | (1 << 7) | (1 << 9) | (1 << 11) | (1 << 20);

unsafe extern "C" {
    fn acl_get_link_np(path: *const c_char, kind: c_int) -> *mut c_void;
    fn acl_get_entry(acl: *mut c_void, index: c_int, entry: *mut *mut c_void) -> c_int;
    fn acl_get_tag_type(entry: *mut c_void, tag: *mut c_int) -> c_int;
    fn acl_get_permset_mask_np(entry: *mut c_void, mask: *mut u64) -> c_int;
    fn acl_free(acl: *mut c_void) -> c_int;
}

pub(super) fn protected(path: &Path) -> bool {
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // An existing file with no ACL reports ENOENT for its ACL property.
    let acl = unsafe { acl_get_link_np(path.as_ptr(), ACL_TYPE_EXTENDED) };
    if acl.is_null() {
        return io::Error::last_os_error().kind() == io::ErrorKind::NotFound;
    }
    let mut index = ACL_FIRST_ENTRY;
    let mut entry = null_mut();
    // Entries borrow the ACL, which remains live until the loop ends.
    let safe = loop {
        if unsafe { acl_get_entry(acl, index, &raw mut entry) } != 0 {
            // Darwin's iterator returns EINVAL when there are no more entries.
            break io::Error::last_os_error().raw_os_error()
                == Some(rustix::io::Errno::INVAL.raw_os_error());
        }
        let mut tag = 0;
        let mut mask = 0;
        if unsafe { acl_get_tag_type(entry, &raw mut tag) } != 0
            || (tag != ACL_EXTENDED_DENY
                && (unsafe { acl_get_permset_mask_np(entry, &raw mut mask) } != 0
                    || mask & !READ_ONLY != 0))
        {
            break false;
        }
        index = ACL_NEXT_ENTRY;
    };
    unsafe {
        acl_free(acl);
    }
    safe
}
