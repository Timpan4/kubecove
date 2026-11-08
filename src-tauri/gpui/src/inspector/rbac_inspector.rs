//! The RBAC inspector card from `RbacView.svelte`: header, evidence tiles,
//! findings, rules, subjects, observed grants and the source chain.

use super::Inspector;
use super::controls::{Variant, button};
use super::rbac_queue::{review_badge, risk_badge, risk_rail};
use super::surface;
use crate::rbac::permissions::{Identity, IdentityKind};
use crate::rbac::{self, CockpitItem, ObjectRef, RbacView};
use crate::targets::TestSupportExt;
use crate::theme::{MONO, Palette, Tokens};
use crate::ui;
use gpui_kit::{ClipboardItem, Context, Div, SharedString, div, prelude::*, px};
use kubecove_lib::models::RbacInspectionSummary;

/// Observed grant rows (summary, provenance, cluster-wide, namespace) and the
/// reason they are unknown, copied out of the observed cache for one frame.
type Grants = Option<(Vec<(String, String, bool, Option<String>)>, Option<String>)>;

/// `selectedSubtitle`.
fn subtitle(category: RbacView) -> &'static str {
    match category {
        RbacView::ServiceAccounts => "identity investigation",
        RbacView::Bindings => "subject and role investigation",
        RbacView::Roles | RbacView::ClusterRoles => "policy investigation",
        RbacView::NamespaceAccess => "namespace access investigation",
    }
}

/// `policyEvidenceLabel` and `policyEvidenceValue`.
fn policy_evidence(data: &RbacInspectionSummary, item: &CockpitItem) -> (&'static str, String) {
    match item.object.get(data) {
        ObjectRef::Account(account) => (
            "Token posture",
            match account.automount_token {
                Some(true) => "Automount on".into(),
                Some(false) => "Automount off".into(),
                None => "Not reported".into(),
            },
        ),
        ObjectRef::Role(role) => ("Policy shape", format!("{} rules", role.rules_count)),
        ObjectRef::Binding(binding) => (
            "Policy shape",
            format!("{} subjects", binding.subjects.len()),
        ),
        ObjectRef::Namespace(access) => ("Policy shape", format!("{} roles", access.roles)),
    }
}

pub(super) fn caps(label: impl Into<SharedString>) -> Div {
    let label: SharedString = label.into();
    ui::caps(&label, 10.).line_height(px(16.25))
}

/// `rounded-md border` with a `border-b px-3 py-2` caps heading.
fn section(heading: impl Into<SharedString>, body: impl IntoElement) -> Div {
    div()
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .child(
            div()
                .px(px(12.))
                .py(px(8.))
                .border_b_1()
                .border_color(Tokens::border())
                .child(caps(heading)),
        )
        .child(body)
}

fn tile(label: &'static str, value: String, detail: &'static str) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .p(px(10.))
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::muted().opacity(0.2))
        .child(caps(label))
        .child(ui::semibold(ui::sm(value)).mt(px(4.)))
        .child(ui::muted(ui::xs(detail)).mt(px(2.)))
}

fn object_label(kind: &str, namespace: Option<&String>, name: &str) -> String {
    format!(
        "{kind}/{}{name}",
        namespace.map_or(String::new(), |ns| format!("{ns}/"))
    )
}

impl Inspector {
    fn copy_rbac_evidence(&mut self, cx: &mut Context<Self>) {
        let state = &self.rbac;
        let (Some(data), Some(item)) = (state.data(), state.selected_item()) else {
            return;
        };
        let json = match item.object.get(data) {
            ObjectRef::Account(value) => serde_json::to_string_pretty(value),
            ObjectRef::Role(value) => serde_json::to_string_pretty(value),
            ObjectRef::Binding(value) => serde_json::to_string_pretty(value),
            ObjectRef::Namespace(value) => serde_json::to_string_pretty(value),
        };
        if let Ok(json) = json {
            cx.write_to_clipboard(ClipboardItem::new_string(json));
        }
    }

    pub(super) fn rbac_inspector(&mut self, cx: &mut Context<Self>) -> Div {
        let identity = self.rbac.active_identity(cx);
        let grants: Grants = self.rbac_observed(cx).map(|observed| {
            (
                observed
                    .grants
                    .iter()
                    .map(|grant| {
                        (
                            grant.summary(),
                            grant.provenance(),
                            grant.cluster,
                            grant.namespace.clone(),
                        )
                    })
                    .collect(),
                observed
                    .unknown
                    .then(|| observed.reason.clone().unwrap_or_default()),
            )
        });
        let state = &self.rbac;
        let data = state.data().expect("ready");
        let selected = state.selected_item();
        let header = div()
            .flex()
            .flex_wrap()
            .items_start()
            .gap(px(12.))
            .px(px(12.))
            .py(px(12.))
            .border_b_1()
            .border_color(Tokens::border())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(caps(selected.map_or("Expert inspector".into(), |item| {
                        format!("{} · {}", item.kind, subtitle(item.category))
                    })))
                    .child(
                        ui::semibold(ui::text(
                            selected.map_or("Select an RBAC object".into(), |item| {
                                format!("{}/{}", item.kind, item.name)
                            }),
                            16.,
                            24.,
                        ))
                        .mt(px(4.))
                        .truncate(),
                    )
                    .child(
                        ui::muted(ui::xs(selected.map_or(
                            "Exact evidence, policy findings, and permission sources".into(),
                            |item| {
                                format!(
                                    "{} · {}",
                                    item.namespace.as_deref().unwrap_or("Cluster scope"),
                                    identity.label()
                                )
                            },
                        )))
                        .mt(px(2.))
                        .truncate(),
                    ),
            )
            .when_some(selected, |header, item| {
                header
                    .child(risk_badge(
                        &item.risks,
                        rbac::risk_summary_label(&item.risks),
                    ))
                    .children(review_badge(data, &state.reviews, item))
            })
            .child(
                button(Variant::Outline, 24., selected.is_none())
                    .id("rbac-copy")
                    .test_support()
                    .child(ui::icon("clipboard", 12., Tokens::foreground()))
                    .child("Copy evidence")
                    .on_click(cx.listener(|this, _, _, cx| this.copy_rbac_evidence(cx))),
            );
        let body = match selected {
            None => ui::muted(ui::sm("Select an RBAC object to inspect evidence."))
                .flex()
                .items_center()
                .justify_center()
                .min_h(px(256.))
                .into_any_element(),
            Some(item) => div()
                .flex()
                .items_start()
                .gap(px(12.))
                .child(self.rbac_evidence(data, item, &identity, grants, cx))
                .child(self.rbac_aside(data, item, &identity, cx))
                .into_any_element(),
        };
        ui::card()
            .py(px(12.))
            .gap(px(12.))
            .flex_basis(px(0.))
            .flex_grow(3.)
            .min_w(px(560.))
            .child(header)
            .child(div().p(px(12.)).child(body))
    }

    fn rbac_evidence(
        &self,
        data: &RbacInspectionSummary,
        item: &CockpitItem,
        identity: &Identity,
        grants: Grants,
        cx: &mut Context<Self>,
    ) -> Div {
        let sources = rbac::binding_sources(data, item);
        let reach = match &grants {
            Some((grants, _)) if grants.iter().any(|grant| grant.2) => "Cluster-wide".to_owned(),
            Some((grants, _)) if !grants.is_empty() => {
                let mut namespaces: Vec<&str> = grants
                    .iter()
                    .filter_map(|grant| grant.3.as_deref())
                    .collect();
                namespaces.sort_unstable();
                namespaces.dedup();
                let count = namespaces.len();
                format!("{count} namespace{}", if count == 1 { "" } else { "s" })
            }
            Some((_, Some(_))) => "Unknown".into(),
            _ => "No grants".into(),
        };
        let (policy_label, policy_value) = policy_evidence(data, item);
        let risks = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(8.))
            .children(item.risks.iter().map(|risk| {
                div()
                    .relative()
                    .overflow_hidden()
                    .p(px(8.))
                    .pl(px(10.))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(Tokens::border())
                    .child(surface::left_rule(risk_rail(std::slice::from_ref(risk))).w(px(3.)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(ui::semibold(ui::xs(risk.label.clone())))
                            .child(risk_badge(
                                std::slice::from_ref(risk),
                                rbac::serde_label(&risk.level),
                            )),
                    )
                    .child(ui::muted(ui::xs(risk.reason.clone())).mt(px(4.)))
                    .into_any_element()
            }));
        let risks = if item.risks.is_empty() {
            risks.child(ui::muted(ui::xs("No policy heuristic flags for this object.")).p(px(8.)))
        } else {
            risks
        };
        let object = item.object.get(data);
        let rules = match object {
            ObjectRef::Role(role) => Some(section(
                "Exact rules",
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .p(px(8.))
                    .children(role.rules.iter().map(|rule| {
                        ui::xs(rbac::rule_text(rule))
                            .font_family(MONO)
                            .px(px(8.))
                            .py(px(6.))
                            .rounded(px(4.))
                            .bg(Tokens::muted().opacity(0.35))
                    })),
            )),
            _ => None,
        };
        let subjects = match object {
            ObjectRef::Binding(binding) => Some(section(
                format!(
                    "Subjects · {}/{}",
                    binding.role_ref_kind, binding.role_ref_name
                ),
                div().flex().flex_col().px(px(8.)).children(
                    binding.subjects.iter().enumerate().map(|(index, subject)| {
                        let label =
                            object_label(&subject.kind, subject.namespace.as_ref(), &subject.name);
                        let identity = Identity {
                            kind: IdentityKind::of_subject(&subject.kind),
                            name: subject.name.clone(),
                            namespace: subject.namespace.clone(),
                            groups: vec![],
                        };
                        div()
                            .id(SharedString::from(format!("rbac-subject:{label}")))
                            .test_support()
                            .aria_label(label.clone())
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .py(px(8.))
                            .when(index > 0, |row| {
                                row.border_t_1().border_color(Tokens::border())
                            })
                            .cursor_pointer()
                            .child(ui::xs(label).flex_1().min_w_0().truncate())
                            .child(ui::muted(ui::xs("Investigate")))
                            .child(ui::icon("chevron-right", 12., Tokens::foreground()))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.investigate_subject(identity.clone(), cx)
                            }))
                    }),
                ),
            )),
            _ => None,
        };
        let (grant_rows, unknown) = grants.unwrap_or_default();
        let grants_empty = grant_rows.is_empty();
        let grant_list = div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .p(px(8.))
            .when_some(unknown, |list, reason| {
                list.child(
                    ui::xs(format!("Unknown · {reason}"))
                        .p(px(8.))
                        .rounded(px(4.))
                        .border_1()
                        .border_color(Palette::amber500(0.3))
                        .bg(Palette::amber500(0.1))
                        .text_color(Palette::amber300()),
                )
            })
            .children(grant_rows.into_iter().enumerate().map(
                |(index, (summary, provenance, _, _))| {
                    div()
                        .id(("rbac-grant", index))
                        .test_support()
                        .aria_label(summary.clone())
                        .p(px(8.))
                        .rounded(px(4.))
                        .border_1()
                        .border_color(Tokens::border())
                        .bg(Tokens::muted().opacity(0.2))
                        .child(ui::xs(summary).font_family(MONO))
                        .child(ui::muted(ui::xs(provenance)).mt(px(4.)))
                },
            ))
            .when(grants_empty, |list| {
                list.child(
                    ui::muted(ui::xs(if identity.name.is_empty() {
                        "Choose an identity to inspect grants."
                    } else {
                        "No observed grants for this identity."
                    }))
                    .p(px(8.)),
                )
            });
        let chain = div()
            .flex()
            .flex_col()
            .px(px(8.))
            .children(sources.iter().enumerate().map(|(index, source)| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .py(px(8.))
                    .when(index > 0, |row| {
                        row.border_t_1().border_color(Tokens::border())
                    })
                    .child(
                        ui::xs(object_label(
                            &source.kind,
                            source.namespace.as_ref(),
                            &source.name,
                        ))
                        .flex_1()
                        .min_w_0()
                        .truncate(),
                    )
                    .child(
                        ui::muted(ui::xs(format!(
                            "{}/{}",
                            source.role_ref_kind, source.role_ref_name
                        )))
                        .truncate(),
                    )
            }));
        let chain = if sources.is_empty() {
            chain.child(ui::muted(ui::xs("No loaded binding sources.")).p(px(8.)))
        } else {
            chain
        };
        let source_count = sources.len();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(tile("Reach", reach, "Observed policy scope"))
                    .child(tile(
                        "Sources",
                        format!(
                            "{source_count} binding{}",
                            if source_count == 1 { "" } else { "s" }
                        ),
                        "Role and binding provenance",
                    ))
                    .child(tile(policy_label, policy_value, "Exact loaded evidence")),
            )
            .child(section("Why this needs review", risks))
            .children(rules)
            .children(subjects)
            .child(
                div()
                    .id("rbac-grants")
                    .rounded(px(8.))
                    .border_1()
                    .border_color(Tokens::border())
                    .child(
                        div()
                            .px(px(12.))
                            .py(px(8.))
                            .border_b_1()
                            .border_color(Tokens::border())
                            .child(caps("Observed RBAC grants"))
                            .child(
                                ui::muted(ui::xs("Policy-derived, not an authorization verdict."))
                                    .mt(px(2.)),
                            ),
                    )
                    .child(grant_list),
            )
            .child(
                div()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(Tokens::border())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .px(px(12.))
                            .py(px(8.))
                            .border_b_1()
                            .border_color(Tokens::border())
                            .child(ui::icon("link-2", 14., Tokens::muted_foreground()))
                            .child(caps("Source chain")),
                    )
                    .child(chain),
            )
    }
}
