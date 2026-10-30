use super::*;
use kube::client::AuthError;

// Failure modes: initial and wrapped refresh failures disclose plugin args,
// environment or output; auth failures become retryable network failures;
// Debug logging bypasses the serialized error's safe message.
#[test]
fn auth_failures_are_safe_in_every_application_representation() {
    for wrapped in [false, true] {
        let auth = AuthError::AuthExecStart(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "synthetic-private-environment",
        ));
        assert_safe(auth, wrapped);
        assert_safe(
            AuthError::AuthExec("synthetic-private-output".into()),
            wrapped,
        );
    }
}

#[cfg(any(unix, windows))]
#[test]
fn failed_process_output_and_command_never_reach_application_errors() {
    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt;

    for wrapped in [false, true] {
        assert_safe(
            AuthError::AuthExecRun {
                cmd: "synthetic-private-command-and-environment".into(),
                status: std::process::ExitStatus::from_raw(1),
                out: std::process::Output {
                    status: std::process::ExitStatus::from_raw(1),
                    stdout: b"synthetic-private-stdout".to_vec(),
                    stderr: b"synthetic-private-stderr".to_vec(),
                },
            },
            wrapped,
        );
    }
}

fn assert_safe(auth: AuthError, wrapped: bool) {
    let error = if wrapped {
        kube::Error::Service(Box::new(auth))
    } else {
        kube::Error::Auth(auth)
    };
    assert_eq!(kube_error_kind(&error), AppErrorKind::CredentialUnavailable);
    let error = AppError::from(error);
    for text in [
        error.to_string(),
        format!("{error:?}"),
        serde_json::to_string(&error).unwrap(),
    ] {
        assert!(!text.contains("synthetic-private"));
    }
    assert!(error.source().is_some());
}
