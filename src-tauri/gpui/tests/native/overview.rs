use super::argo::scroll;
use super::*;
use gpui_kit::test::TestQueryExt;

fn click(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle, id: &str) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        if !window.find(id.to_owned()).visible() {
            scroll::reveal(window, "page", id, cx);
        }
        window.click(id.to_owned(), cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn refresh(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) {
    click(cx, handle, "open-resources");
    click(cx, handle, "nav:Workspace Overview");
}

#[gpui_kit::test]
fn empty_successful_read_clamps_findings_before_they_regrow(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.workspace.enabled.store(true, Ordering::SeqCst);
    let rows: Vec<ResourceSummary> = (0..=kubecove_gpui::model::PAGE_SIZE).map(|index| {
        serde_json::from_value(json!({
            "kind":"Pod", "cluster":"mock-dev", "namespace":"payments", "age":"1d",
            "name":format!("finding-{index:03}"), "apiVersion":"v1",
            "healthAssessment":{"state":"degraded","completeness":"complete","winningSources":["kubernetes"],"reasons":[],"evidence":[]}
        })).unwrap()
    }).collect();
    *backend.workspace.rows.lock().unwrap() = Some(rows.clone());
    let handle = open(cx, backend.clone());
    click(cx, handle, "nav:Workspace Overview");
    click(cx, handle, "overview-next");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .try_find_by_label("Pod / finding-050 in payments on mock-dev")
                .is_some()
        );
    })
    .unwrap();

    *backend.workspace.rows.lock().unwrap() = Some(vec![]);
    refresh(cx, handle);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("overview-description:Resource Health").label(),
            Some("0 resources loaded from the saved scope.")
        );
    })
    .unwrap();

    *backend.workspace.rows.lock().unwrap() = Some(rows);
    refresh(cx, handle);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("overview-description:Resource Health").label(),
            Some("51 resources loaded from the saved scope.")
        );
        assert!(
            window
                .try_find_by_label("Pod / finding-000 in payments on mock-dev")
                .is_some()
        );
        assert!(
            window
                .try_find_by_label("Pod / finding-050 in payments on mock-dev")
                .is_none()
        );
    })
    .unwrap();
}
