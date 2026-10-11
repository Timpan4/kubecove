# ADR 0021: File Credential Store Without a Keyring

## Status

Accepted 2026-10-09. Amends [ADR 0013](0013-argocd-connected-inspection-and-operations.md) for `kubecove-server` only.

## Context

ADR 0013 stores remembered connected Argo CD credentials and five-minute operation review sessions only in native credential storage. It forbids plaintext and memory-only fallbacks, so operation review becomes unavailable without a keyring. Many remote Linux hosts that run [ADR 0020](0020-headless-server-mode.md) server mode have no Secret Service, which would leave Argo CD operations unavailable there.

## Decision

When `kubecove-server` starts, it probes native credential storage by writing, reading, and deleting a probe entry. If the probe succeeds, ADR 0013 applies unchanged. If it fails because no secure storage is available, the server uses a file store for the same two items: remembered connected credentials, which still require explicit user opt-in, and operation review sessions.

File store rules:

- Data lives in `credentials.enc`, and the key lives in `credentials.key`, both in the data directory. The key is 32 random bytes from the OS CSPRNG, the key size required by the AEAD cipher.
- Each entry is sealed separately with an AEAD cipher, a fresh random nonce, and the entry's service and id as associated data.
- Files are written atomically by writing a temporary file and renaming it, with file mode `0600` in a directory with mode `0700`. At startup the server refuses the file store if either file is readable by group or others or is owned by another user.
- The file store is Unix-only, including macOS when its keychain is unavailable. The server refuses to start a file store on other platforms.
- Expired review sessions are purged when read and at startup. Consumed sessions are removed before execution, matching ADR 0013's single-use rule. Forgetting a credential preserves other entries, including other users' pending reviews. Both files are deleted only when the store has no remaining entries.
- Settings shows which store is active. With the file store, it says that credentials are stored in a file on this host.

## Consequences

Argo CD connected inspection and operations work on hosts without a keyring. The file store protects against other OS users and against copying only one of the two files. It does not protect against processes running as the same user, root, or backups that contain both files, so its practical protection is close to a `0600` plaintext file. The desktop app keeps ADR 0013's rule: no fallback, and operation review is unavailable without native storage.
