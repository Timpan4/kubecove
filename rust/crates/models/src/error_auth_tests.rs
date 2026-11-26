use super::*;
use crate::credential::{CredentialIdentity, CredentialPluginError};
use kube::client::AuthError;

// Failure modes: initial and wrapped refresh failures disclose plugin args,
// environment or stdout; auth failures become retryable network failures;
// Debug logging bypasses the serialized error's safe message; an untagged
// error (server sessions) gains plugin detail; a tagged refresh loses it.
#[test]
fn auth_failures_are_safe_in_every_application_representation() {
    for tagged in [false, true] {
        for wrapped in [false, true] {
            let auth = AuthError::AuthExecStart(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "synthetic-private-environment",
            ));
            assert_safe(auth, wrapped, tagged);
            assert_safe(
                AuthError::AuthExec("synthetic-private-output".into()),
                wrapped,
                tagged,
            );
        }
    }
}

#[cfg(any(unix, windows))]
fn failed_run(stderr: &[u8]) -> AuthError {
    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt;
    AuthError::AuthExecRun {
        cmd: "synthetic-private-command-and-environment".into(),
        out: Box::new(std::process::Output {
            // Unix encodes exit codes in the wait status's high byte.
            #[cfg(unix)]
            status: std::process::ExitStatus::from_raw(1 << 8),
            #[cfg(windows)]
            status: std::process::ExitStatus::from_raw(1),
            stdout: b"synthetic-private-stdout".to_vec(),
            stderr: stderr.to_vec(),
        }),
    }
}

#[cfg(any(unix, windows))]
#[test]
fn failed_process_stdout_and_command_never_reach_application_errors() {
    for tagged in [false, true] {
        for wrapped in [false, true] {
            assert_safe(
                failed_run(b"token=synthetic-private-stderr"),
                wrapped,
                tagged,
            );
        }
    }
}

#[cfg(any(unix, windows))]
#[test]
fn tagged_plugin_failure_carries_identity_exit_status_and_redacted_line() {
    let error = AppError::from(kube::Error::Service(Box::new(CredentialPluginError::new(
        Arc::new(identity()),
        Box::new(failed_run(
            b"opening browser\nerror: issuer did not match bearer synthetic-private-bearer\n",
        )),
    ))));
    assert_eq!(error.kind, AppErrorKind::CredentialUnavailable);
    let credential = error.credential.as_deref().expect("tagged failure");
    assert_eq!(credential.context, "SG1");
    assert_eq!(credential.user.as_deref(), Some("oidc"));
    assert_eq!(credential.command, "kubectl oidc-login get-token");
    assert_eq!(credential.exit_status, Some(1));
    assert_eq!(
        credential.plugin_message.as_deref(),
        Some("error: issuer did not match bearer [REDACTED]")
    );
    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(json["credential"]["exitStatus"], 1);
    assert_eq!(
        json["credential"]["reproduce"],
        "kubectl --context SG1 auth whoami"
    );
    assert!(!format!("{error:?}").contains("issuer did not match"));
}

#[cfg(any(unix, windows))]
#[test]
fn client_build_failure_gains_identity_and_untagged_errors_stay_plain() {
    let build = AppError::from(kube::Error::Auth(failed_run(b"error: expired\n")));
    assert!(build.credential.is_none());
    assert!(
        serde_json::to_value(&build)
            .unwrap()
            .get("credential")
            .is_none()
    );
    let described = build.with_credential_identity(&identity());
    assert_eq!(
        described
            .credential
            .as_deref()
            .and_then(|credential| credential.plugin_message.as_deref()),
        Some("error: expired")
    );
    let network =
        AppError::new("offline", AppErrorKind::Network).with_credential_identity(&identity());
    assert!(network.credential.is_none());
}

fn identity() -> CredentialIdentity {
    CredentialIdentity {
        context: "SG1".into(),
        user: Some("oidc".into()),
        command: "kubectl oidc-login get-token".into(),
        kubeconfig: vec!["/home/user/.kube/config".into()],
        reproduce: "kubectl --context SG1 auth whoami".into(),
    }
}

fn assert_safe(auth: AuthError, wrapped: bool, tagged: bool) {
    let source: Box<dyn Error + Send + Sync> = Box::new(auth);
    let source = if tagged {
        Box::new(CredentialPluginError::new(Arc::new(identity()), source))
    } else {
        source
    };
    let error = if wrapped || tagged {
        kube::Error::Service(source)
    } else {
        kube::Error::Auth(*source.downcast::<AuthError>().unwrap())
    };
    assert_eq!(kube_error_kind(&error), AppErrorKind::CredentialUnavailable);
    let error = AppError::from(error);
    assert_eq!(error.credential.is_some(), tagged);
    for text in [
        error.to_string(),
        format!("{error:?}"),
        serde_json::to_string(&error).unwrap(),
    ] {
        assert!(!text.contains("synthetic-private"), "{text}");
    }
    assert!(error.source().is_some());
}
