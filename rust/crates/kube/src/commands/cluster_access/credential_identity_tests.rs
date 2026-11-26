use super::*;
use crate::models::{AppError, AppErrorKind};

// Failure modes: the command row leaks a flag value, path, token or other
// positional argument; a context or path name breaks out of the copied shell
// command; the copied command reads a different kubeconfig than KubeCove; a
// tagged layer turns a non-auth failure into a credential failure or drops
// its kind.

fn exec(command: &str, args: &[&str]) -> ExecConfig {
    serde_json::from_value(serde_json::json!({
        "apiVersion": "client.authentication.k8s.io/v1",
        "command": command,
        "args": args,
    }))
    .unwrap()
}

#[test]
fn command_shows_known_subcommands_and_never_other_arguments() {
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
        (
            "/opt/kubelogin.exe",
            &["get-token", "--server-id", "x"][..],
            "kubelogin.exe get-token",
        ),
        ("gke-gcloud-auth-plugin", &[][..], "gke-gcloud-auth-plugin"),
        // Positional values that look like subcommand words.
        ("plugin", &["secret-token", "get"][..], "plugin"),
        ("kubectl", &["oidc-login", "secret-token"][..], "kubectl"),
        ("aws", &["eks"][..], "aws"),
    ] {
        assert_eq!(super::command(&exec(command, args)), expected);
    }
}

#[test]
fn reproduce_quotes_values_and_names_exactly_the_loaded_files() {
    let default = PathBuf::from("/home/u/.kube/config");
    let one = std::slice::from_ref(&default);
    // The default file is named too, so the terminal's KUBECONFIG cannot
    // pick another user for the context.
    assert_eq!(
        reproduce("SG1", one, false).as_deref(),
        Some("kubectl --kubeconfig /home/u/.kube/config --context SG1 auth whoami")
    );
    assert_eq!(
        reproduce("it's $(x)", one, false).as_deref(),
        Some(r"kubectl --kubeconfig /home/u/.kube/config --context 'it'\''s $(x)' auth whoami")
    );
    // PowerShell ends single-quoted strings at typographic quotes too.
    let other = PathBuf::from("/srv/team’s config");
    assert_eq!(
        reproduce("team’s ‘x’ it's", std::slice::from_ref(&other), true).as_deref(),
        Some(
            "kubectl --kubeconfig '/srv/team’’s config' --context 'team’’s ‘‘x’’ it''s' auth whoami"
        )
    );
    let list = if cfg!(windows) { ';' } else { ':' };
    assert_eq!(
        reproduce("dev", &[other.clone(), default.clone()], false),
        Some(format!(
            "KUBECONFIG='/srv/team’s config{list}/home/u/.kube/config' kubectl --context dev auth whoami"
        ))
    );
    // A name the platform cannot list, and no files at all, give no command
    // rather than one that falls back to the terminal's kubeconfig.
    let unlistable = PathBuf::from(if cfg!(windows) { "a\"b" } else { "a:b" });
    assert_eq!(
        reproduce("dev", &[unlistable, default.clone()], false),
        None
    );
    assert_eq!(reproduce("dev", &[], false), None);
}

#[tokio::test]
async fn tag_layer_wraps_only_authentication_failures() {
    use tower::ServiceExt;
    let identity = Arc::new(CredentialIdentity {
        context: "SG1".into(),
        user: None,
        command: "kubectl".into(),
        kubeconfig: Vec::new(),
        reproduce: Some("kubectl --kubeconfig /k --context SG1 auth whoami".into()),
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
