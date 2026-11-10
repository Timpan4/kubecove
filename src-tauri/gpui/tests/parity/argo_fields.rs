//! Text fields of the Argo CD connection dialog, which each dialog keeps for itself.
use super::*;

/// The input id of a field the scenarios name, as the web's label names it.
fn field_id(field: &str) -> &'static str {
    match field {
        "url" => "argo-url",
        "rootPath" => "argo-root-path",
        "tlsServerName" => "argo-tls-name",
        "token" => "argo-token",
        "password" => "argo-password",
        other => panic!("unknown Argo connection field {other}"),
    }
}

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) {
    let id = field_id(string(step, "field"));
    // Both dialogs keep their fields in the open dialog, and it is the only container the steps need.
    let anchor = if window.try_find("argo-connection-dialog").is_some() {
        "argo-connection-dialog"
    } else {
        "detail-body"
    };
    reveal(window, anchor, id, cx);
    if string(step, "action") == "fillArgoConnectionField" {
        replace_text(window, cx, id, string(step, "value"));
        return;
    }
    window.render_frame(cx);
    // The message omits the value, which can be a credential.
    let field = string(step, "field");
    let actual = if matches!(field, "token" | "password") {
        // Masked inputs expose no accessibility value, so the Inspector answers, as the web
        // driver reads the input's value.
        gpui_kit::base::Root::read(window, cx)
            .view()
            .clone()
            .downcast::<kubecove_gpui::Inspector>()
            .expect("the Inspector is the window's content")
            .read(cx)
            .argo_typed_secret(field, cx)
    } else {
        window.find(id).value().unwrap_or("").to_owned()
    };
    assert!(
        actual == string(step, "value"),
        "{id} does not hold the expected value"
    );
}
