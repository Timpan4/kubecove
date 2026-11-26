use super::*;
use crate::models::{AppError, AppErrorKind};

// Failure modes: the command row leaks a flag value, path or token argument;
// a context or path name breaks out of the copied shell command; the copied
// command reads a different kubeconfig than KubeCove; a tagged layer turns
// a non-auth failure into a credential failure or drops its kind.

fn exec(command: &str, args: &[&str]) -> ExecConfig {
    serde_json::from_value(serde_json::json!({
        "apiVersion": "client.authentication.k8s.io/v1",
        "command": command,
        "args": args,
    }))
    .unwrap()
}

#[test]
fn command_keeps_program_name_and_leading_subcommands_only() {
    for (command, args, expected) in [
        (
            "kubectl",
            &[
                "oidc-login",
                "get-token",
                "--oidc-issuer-url=https://idp",
                "secret",
            ][..],
            "kubectl oidc-login get-token",
        ),
        (
            "/opt/bin/aws",
            &["eks", "get-token", "--cluster-name", "x"][..],
            "aws eks get-token",
        ),
        ("gke-gcloud-auth-plugin", &[][..], "gke-gcloud-auth-plugin"),
        ("plugin", &["Token123", "get"][..], "plugin"),
        (
            "plugin",
            &["get", "/home/user/key", "more"][..],
            "plugin get",
        ),
        ("plugin", &["get", "tok=abc"][..], "plugin get"),
    ] {
        assert_eq!(super::command(&exec(command, args)), expected);
    }
}

#[test]
fn reproduce_quotes_context_and_names_non_default_kubeconfig() {
    let default = PathBuf::from("/home/u/.kube/config");
    assert_eq!(
        reproduce("SG1", std::slice::from_ref(&default), Some(&default), false),
        "kubectl --context SG1 auth whoami"
    );
    assert_eq!(
        reproduce("it's $(x)", &[], Some(&default), false),
        r"kubectl --context 'it'\''s $(x)' auth whoami"
    );
    assert_eq!(
        reproduce("it's $x", &[], None, true),
        "kubectl --context 'it''s $x' auth whoami"
    );
    let other = PathBuf::from("/srv/team config");
    assert_eq!(
        reproduce("arn:aws:eks", &[other], Some(&default), false),
        "KUBECONFIG='/srv/team config' kubectl --context 'arn:aws:eks' auth whoami"
    );
    // `join_paths` rejects a drive colon on Unix, where `:` separates paths.
    #[cfg(windows)]
    assert_eq!(
        reproduce("dev", &[PathBuf::from(r"C:\k\config")], None, true),
        r"$env:KUBECONFIG='C:\k\config'; kubectl --context dev auth whoami"
    );
}

#[tokio::test]
async fn tag_layer_wraps_only_authentication_failures() {
    use tower::ServiceExt;
    let identity = Arc::new(CredentialIdentity {
        context: "SG1".into(),
        user: None,
        command: "kubectl".into(),
        kubeconfig: Vec::new(),
        reproduce: "kubectl --context SG1 auth whoami".into(),
    });
    let fail = |error: BoxError| {
        let error = Arc::new(std::sync::Mutex::new(Some(error)));
        CredentialTagLayer::new(Some(identity.clone())).layer(tower::service_fn(
            move |_: Request<()>| {
                let error = error.lock().unwrap().take().unwrap();
                async move { Err::<(), BoxError>(error) }
            },
        ))
    };
    let auth: BoxError = Box::new(kube::client::AuthError::AuthExec("x".into()));
    let error = fail(auth).oneshot(Request::new(())).await.unwrap_err();
    let error = AppError::from(kube::Error::Service(error));
    assert_eq!(error.kind, AppErrorKind::CredentialUnavailable);
    assert_eq!(
        error.credential.as_deref().map(|c| c.context.as_str()),
        Some("SG1")
    );

    let io: BoxError = Box::new(std::io::Error::from(std::io::ErrorKind::ConnectionRefused));
    let error = fail(io).oneshot(Request::new(())).await.unwrap_err();
    let error = AppError::from(kube::Error::Service(error));
    assert_eq!(error.kind, AppErrorKind::Network);
    assert!(error.credential.is_none());
}
