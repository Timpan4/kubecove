//! Settings and RBAC controls against the in-memory fixture backend.
use super::*;
const RBAC_CATEGORIES: &[&str] = &[
    "Namespace Access",
    "Roles",
    "Cluster Roles",
    "Bindings",
    "Service Accounts",
];

pub(super) static CASES: &[Case] = &[
    Case {
        id: "settings-diagnostics-toggle",
        description: "Toggle diagnostics and rebuild its command timing view",
        setup: |d| views::setup(d, "settings-diagnostics"),
        prepare: None,
        aim: |w, _| center(w, "setting:Enable diagnostics"),
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "settings-agents-toggle",
        description: "Toggle the mock MCP server and render its returned status",
        setup: |d| views::setup(d, "settings-agents"),
        prepare: None,
        aim: |w, _| center(w, "mcp-control:MCP server"),
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "settings-agents-port-typing",
        description: "Type and erase the MCP port without saving",
        setup: |d| {
            views::setup(d, "settings-agents");
            d.act(|w, cx| {
                w.click("mcp-port", cx);
                w.press("ctrl-a", cx);
                w.press("backspace", cx);
            });
        },
        prepare: None,
        aim: nowhere,
        step: Some(|w, cx, i, _| typing(w, cx, i, "19090")),
        fixture: None,
        replay: None,
        probe: Some(|w| value(w, "mcp-port")),
    },
    Case {
        id: "rbac-category-navigation",
        description: "Cycle all RBAC categories and their inspection queues",
        setup: |d| {
            views::setup(d, "rbac");
            d.act(|w, cx| w.click("rbac-category:Namespace Access", cx));
        },
        prepare: None,
        aim: |w, i| {
            let category = RBAC_CATEGORIES[(i + 1) % RBAC_CATEGORIES.len()];
            center(w, &format!("rbac-category:{category}"))
        },
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
];
