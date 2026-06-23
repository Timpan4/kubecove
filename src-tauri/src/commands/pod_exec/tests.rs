use super::*;
use std::collections::HashSet;

fn valid_request() -> PodExecSessionRequest {
    PodExecSessionRequest {
        cluster_context: "kind-dev".to_string(),
        kubeconfig_env_var: None,
        namespace: "default".to_string(),
        pod_name: "api-0".to_string(),
        container: Some("api".to_string()),
        command: vec!["/bin/sh".to_string()],
        stdin: true,
        tty: true,
        terminal_size: PodExecTerminalSize {
            cols: 100,
            rows: 32,
        },
        confirmation: crate::models::PodExecConfirmation {
            acknowledged: true,
            target: "kind-dev/default/Pod/api-0/container/api".to_string(),
            command: "[\"/bin/sh\"]".to_string(),
        },
    }
}

fn test_summary(id: &str) -> PodExecSessionSummary {
    PodExecSessionSummary {
        id: id.to_string(),
        cluster_context: "kind-dev".to_string(),
        kubeconfig_env_var: None,
        kubeconfig_source_key: None,
        kubeconfig_source_label: None,
        namespace: "default".to_string(),
        pod_name: "api-0".to_string(),
        container: Some("api".to_string()),
        command: vec!["/bin/sh".to_string()],
        stdin: true,
        tty: true,
        terminal_cols: 100,
        terminal_rows: 32,
        status: "running".to_string(),
        started_at: "2026-06-01T00:00:00Z".to_string(),
        finished_at: None,
        exit_code: None,
        last_error: None,
    }
}

#[test]
fn validates_target_command_confirmation_and_terminal_size() {
    assert!(validate_request(&valid_request()).is_ok());

    assert_eq!(
        validate_request(&PodExecSessionRequest {
            pod_name: " ".to_string(),
            ..valid_request()
        })
        .expect_err("missing target")
        .message,
        "pod exec target is required",
    );
    assert_eq!(
        validate_request(&PodExecSessionRequest {
            command: vec![],
            ..valid_request()
        })
        .expect_err("missing command")
        .message,
        "pod exec command is required",
    );
    assert_eq!(
        validate_request(&PodExecSessionRequest {
            confirmation: crate::models::PodExecConfirmation {
                acknowledged: false,
                ..valid_request().confirmation
            },
            ..valid_request()
        })
        .expect_err("missing confirmation")
        .message,
        "pod exec requires explicit confirmation",
    );
    assert_eq!(
        validate_request(&PodExecSessionRequest {
            terminal_size: PodExecTerminalSize { cols: 0, rows: 24 },
            ..valid_request()
        })
        .expect_err("bad size")
        .message,
        "terminal size must be between 1 and 500 columns and rows",
    );
}

#[test]
fn registry_lists_marks_resizes_and_stops_sessions() {
    let registry = PodExecRegistry::default();
    registry.insert_summary_for_test(test_summary("exec-1"));

    assert_eq!(registry.list().len(), 1);
    registry.mark_error("exec-1", "forbidden".to_string());
    let session = registry.list().pop().expect("session");
    assert_eq!(session.status, "error");
    assert_eq!(session.last_error.as_deref(), Some("forbidden"));

    registry.mark_terminal_size(
        "exec-1",
        PodExecTerminalSize {
            cols: 120,
            rows: 40,
        },
    );
    let session = registry.list().pop().expect("session");
    assert_eq!(session.terminal_cols, 120);
    assert_eq!(session.terminal_rows, 40);

    registry.mark_exited("exec-1", Some(0));
    let session = registry.list().pop().expect("session");
    assert_eq!(session.status, "exited");
    assert_eq!(session.exit_code, Some(0));

    assert!(registry.stop("exec-1"));
    assert!(!registry.stop("exec-1"));
    assert!(registry.list().is_empty());
}

#[test]
fn registry_stops_sessions_outside_context_or_source_scope() {
    let registry = PodExecRegistry::default();
    let mut kept = test_summary("exec-kept");
    kept.kubeconfig_source_key = Some("kubeconfigSource=current".to_string());
    let mut wrong_context = test_summary("exec-context");
    wrong_context.cluster_context = "other-context".to_string();
    wrong_context.kubeconfig_source_key = Some("kubeconfigSource=current".to_string());
    let mut wrong_source = test_summary("exec-source");
    wrong_source.kubeconfig_source_key = Some("kubeconfigSource=old".to_string());
    registry.insert_summary_for_test(kept);
    registry.insert_summary_for_test(wrong_context);
    registry.insert_summary_for_test(wrong_source);

    let allowed = HashSet::from(["kind-dev".to_string()]);
    let stopped = registry.stop_outside_scope(&allowed, "kubeconfigSource=current");

    assert_eq!(
        stopped,
        vec!["exec-context".to_string(), "exec-source".to_string()]
    );
    let remaining = registry.list();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, "exec-kept");
}
