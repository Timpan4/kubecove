//! Metadata sections of the resource Details tab: the owner header, the
//! ownership card, label and annotation badges with Show more/less, the
//! Annotations and Management card and Advanced metadata. Mirrors the matching
//! parts of `DetailsTab.svelte` and `metadata-details.ts`. The Containers table
//! lives in `containers.rs`.

use super::super::Inspector;
use super::super::detail::{Detail, ResourceDetail};
use super::super::helm_page::HelmTarget;
use super::containers::{cell, row_of, table_body};
use super::metadata_card;
use crate::targets::TestSupportExt;
use crate::theme::{MONO, Tokens};
use crate::ui;
use gpui_kit::{Context, Div, FontWeight, Hsla, div, prelude::*, px};
use kubecove_lib::models::{GitOpsOwnerSummary, ResourceSummary};
use serde_json::Value;

/// `visibleMetadataBadges` default limit, shared by labels and annotations.
const BADGE_LIMIT: usize = 4;
/// `isInlineAnnotationValue`: longer or multi-line values stay in Advanced metadata.
const INLINE_ANNOTATION_MAX: usize = 96;
/// The managers list shows this many names before a `+N more` count.
const MANAGER_LIMIT: usize = 4;

/// Expansion state for the metadata sections, which resets with the panel.
#[derive(Default)]
pub(in crate::inspector) struct MetadataDisclosure {
    labels: bool,
    annotations: bool,
    advanced: bool,
}

#[derive(Clone, Copy)]
enum Disclosure {
    Labels,
    Annotations,
    Advanced,
}

impl Inspector {
    /// `openHelmRelease`: the Helm page selects the release once it loads.
    fn open_helm_release(
        &mut self,
        name: String,
        namespace: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.helm_page.target = Some(HelmTarget { name, namespace });
        self.open_page(super::super::Page::Helm, cx);
    }

    fn toggle_disclosure(&mut self, disclosure: Disclosure, cx: &mut Context<Self>) {
        if let Detail::Ready(detail) = &mut self.detail {
            let state = &mut detail.metadata;
            match disclosure {
                Disclosure::Labels => state.labels = !state.labels,
                Disclosure::Annotations => state.annotations = !state.annotations,
                Disclosure::Advanced => state.advanced = !state.advanced,
            }
        }
        cx.notify();
    }
}

/// The Owner badge after the restarts badge in the Details header.
pub(super) fn owner_badge(summary: &ResourceSummary) -> Option<impl IntoElement> {
    let owner = summary
        .owner_ref
        .as_deref()
        .filter(|owner| !owner.is_empty())?;
    let label = format!("Owner {owner}");
    Some(
        ui::outline_badge(label.clone())
            .id("header-owner")
            .test_support()
            .aria_label(label),
    )
}

/// The header's Open Helm release button, shown when the resource has a release.
pub(super) fn helm_header_button(
    summary: &ResourceSummary,
    cx: &mut Context<Inspector>,
) -> Option<impl IntoElement> {
    let release = summary
        .helm_release
        .clone()
        .filter(|name| !name.is_empty())?;
    let namespace = summary.namespace.clone();
    Some(
        ui::outline_button()
            .h(px(28.))
            .px(px(12.))
            .gap(px(6.))
            .id("header-open-helm")
            .test_support()
            .aria_label("Open Helm release")
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_helm_release(release.clone(), namespace.clone(), cx)
            }))
            .child(ui::icon("external-link", 12., Tokens::foreground()))
            .child(ui::xs("Open Helm release")),
    )
}

/// `Ownership` card: controller owner fields, GitOps evidence and Helm release.
pub(super) fn ownership_card(detail: &ResourceDetail, cx: &mut Context<Inspector>) -> Div {
    let summary = &detail.data.summary;
    let mut owner = owner_fields(&detail.data.metadata);
    if owner.is_empty()
        && let Some(name) = summary.owner_ref.clone().filter(|name| !name.is_empty())
    {
        owner.push(("Owner", name));
    }
    let mut rows: Vec<Div> = owner
        .into_iter()
        .map(|(label, text)| field(label, labeled_value(label, text)))
        .collect();
    rows.extend(
        gitops_rows(summary)
            .into_iter()
            .map(|(label, text)| field(label, labeled_value(label, text))),
    );
    if let Some(release) = summary.helm_release.clone().filter(|name| !name.is_empty()) {
        let namespace = summary.namespace.clone();
        rows.push(field("Helm", helm_row_button(release, namespace, cx)));
    }
    if rows.is_empty() {
        rows.push(ui::muted(ui::xs("None detected")));
    }
    metadata_card("Ownership", rows)
}

/// `Labels` card; the badges show four values until the user expands them.
pub(super) fn labels_card(detail: &ResourceDetail, cx: &mut Context<Inspector>) -> Div {
    let labels = metadata_entries(&detail.data.metadata, "labels");
    let body = if labels.is_empty() {
        ui::muted(ui::xs("None"))
    } else {
        badge_block(
            "labels",
            &labels,
            detail.metadata.labels,
            Disclosure::Labels,
            cx,
        )
    };
    metadata_card("Labels", vec![body])
}

/// `Annotations and Management` card: the annotation count, managers, inline
/// annotation badges and the note for values that stay in Advanced metadata.
pub(super) fn annotations_card(detail: &ResourceDetail, cx: &mut Context<Inspector>) -> Div {
    let metadata = &detail.data.metadata;
    let annotations = metadata_entries(metadata, "annotations");
    let inline: Vec<(String, String)> = annotations
        .iter()
        .filter(|(_, value)| is_inline_annotation(value))
        .cloned()
        .collect();
    let mut rows = vec![field(
        "Annotations",
        labeled_value("Annotations", annotations.len().to_string()),
    )];
    if let Some(managers) = managers_summary(metadata) {
        rows.push(field("Managers", labeled_value("Managers", managers)));
        rows.push(field(
            "Raw managedFields",
            labeled_value("Raw managedFields", "advanced metadata".to_owned()),
        ));
    }
    let mut card = metadata_card("Annotations and Management", rows);
    if !inline.is_empty() {
        card = card.child(div().mt(px(8.)).child(badge_block(
            "annotations",
            &inline,
            detail.metadata.annotations,
            Disclosure::Annotations,
            cx,
        )));
    }
    let hidden_from_badges = annotations.len() - inline.len();
    if hidden_from_badges > 0 {
        let plural = if hidden_from_badges == 1 { "" } else { "s" };
        let note = format!(
            "{hidden_from_badges} long or structured annotation{plural} in Advanced metadata."
        );
        card = card.child(
            ui::muted(ui::text(note.clone(), 11., 16.))
                .mt(px(8.))
                .id("annotations-note")
                .test_support()
                .aria_label(note),
        );
    }
    card
}

/// `Advanced metadata`: a disclosure with a table of every top-level metadata field.
pub(super) fn advanced_metadata(detail: &ResourceDetail, cx: &mut Context<Inspector>) -> Div {
    let open = detail.metadata.advanced;
    let toggle = div()
        .flex()
        .items_center()
        .px(px(12.))
        .py(px(8.))
        .id("advanced-metadata-toggle")
        .test_support()
        .aria_label("Advanced metadata")
        .cursor_pointer()
        .on_click(cx.listener(|this, _, _, cx| this.toggle_disclosure(Disclosure::Advanced, cx)))
        .child(ui::muted(ui::semibold(ui::caps("Advanced metadata", 12.))));
    let mut block = div()
        .overflow_hidden()
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::background().opacity(0.3))
        .child(toggle);
    if open {
        let rows = raw_rows(&detail.data.metadata)
            .into_iter()
            .map(|(key, text)| {
                row_of(vec![
                    cell(
                        ui::xs(key.clone())
                            .id(format!("advanced-key:{key}"))
                            .test_support()
                            .aria_label(key),
                    ),
                    cell(div().font_family(MONO).text_size(px(11.)).child(text)),
                ])
            })
            .collect();
        block = block.child(
            div()
                .border_t_1()
                .border_color(Tokens::border())
                .bg(Tokens::background().opacity(0.8))
                .child(table_body(&["Metadata", "Value"], rows)),
        );
    }
    block
}

fn helm_row_button(
    release: String,
    namespace: Option<String>,
    cx: &mut Context<Inspector>,
) -> impl IntoElement {
    let label = release.clone();
    ui::outline_button()
        .h(px(28.))
        .px(px(10.))
        .gap(px(6.))
        .id("ownership-open-helm")
        .test_support()
        .aria_label(label.clone())
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _, cx| {
            this.open_helm_release(release.clone(), namespace.clone(), cx)
        }))
        .child(ui::icon("external-link", 12., Tokens::foreground()))
        .child(ui::xs(label))
}

/// A badge list with Show more or Show less when the list is longer than the limit.
fn badge_block(
    prefix: &str,
    items: &[(String, String)],
    expanded: bool,
    disclosure: Disclosure,
    cx: &mut Context<Inspector>,
) -> Div {
    let (shown, hidden) = visible_badges(items, expanded);
    let mut column = div()
        .flex()
        .flex_col()
        .items_start()
        .gap(px(6.))
        .w_full()
        .min_w_0();
    for (key, value) in shown {
        column = column.child(badge(prefix, key, value));
    }
    if hidden == 0 && !expanded {
        return column;
    }
    let label = if expanded {
        "Show less".to_owned()
    } else {
        format!("Show {hidden} more")
    };
    column.child(
        div()
            .mt(px(8.))
            .h(px(28.))
            .px(px(8.))
            .flex()
            .items_center()
            .rounded(px(6.))
            .id(format!("{prefix}-toggle"))
            .test_support()
            .aria_label(label.clone())
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_disclosure(disclosure, cx)))
            .child(ui::xs(label)),
    )
}

/// `visibleMetadataBadges`: the shown entries and how many are hidden.
fn visible_badges(items: &[(String, String)], expanded: bool) -> (&[(String, String)], usize) {
    if expanded || items.len() <= BADGE_LIMIT {
        return (items, 0);
    }
    (&items[..BADGE_LIMIT], items.len() - BADGE_LIMIT)
}

/// A key=value badge tinted by a hash of its key, as `metadataBadgeStyle`.
fn badge(prefix: &str, key: &str, value: &str) -> impl IntoElement {
    let hue = hue_of(key) as f32 / 360.;
    let id = format!("{prefix}-badge:{key}");
    div()
        .flex()
        .items_center()
        .gap(px(4.))
        .min_w_0()
        .max_w_full()
        .px(px(8.))
        .py(px(4.))
        .rounded(px(4.))
        .border_1()
        .border_color(Hsla {
            h: hue,
            s: 0.72,
            l: 0.52,
            a: 0.46,
        })
        .bg(Hsla {
            h: hue,
            s: 0.70,
            l: 0.22,
            a: 0.38,
        })
        .text_color(Hsla {
            h: hue,
            s: 0.88,
            l: 0.78,
            a: 1.,
        })
        .text_size(px(11.))
        .line_height(px(16.))
        .child(
            div()
                .flex_none()
                .font_weight(FontWeight::SEMIBOLD)
                .id(format!("{id}:key"))
                .test_support()
                .aria_label(key.to_owned())
                .child(key.to_owned()),
        )
        .child(div().flex_none().opacity(0.75).child("="))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .id(format!("{id}:value"))
                .test_support()
                .aria_label(format!("{key} value"))
                .child(value.to_owned()),
        )
}

/// `djb2`-style hash of the key's UTF-16 units, reduced to a hue, as the web does.
fn hue_of(key: &str) -> u32 {
    key.encode_utf16().fold(0u32, |hash, unit| {
        hash.wrapping_mul(31).wrapping_add(u32::from(unit))
    }) % 360
}

/// `metadataBadgeEntries`: non-empty string, number or boolean metadata values, by key.
fn metadata_entries(metadata: &Value, field: &str) -> Vec<(String, String)> {
    let Some(map) = metadata.get(field).and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut entries: Vec<(String, String)> = map
        .iter()
        .filter_map(|(key, value)| primitive_text(value).map(|text| (key.clone(), text)))
        .filter(|(_, text)| !text.is_empty())
        .collect();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries
}

fn primitive_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

/// `isInlineAnnotationValue`.
fn is_inline_annotation(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed.chars().count() <= INLINE_ANNOTATION_MAX
        && !trimmed.contains('\n')
        && !trimmed.starts_with('{')
        && !trimmed.starts_with('[')
}

/// `managedFieldsSummary`: distinct managers, then a count of the rest.
fn managers_summary(metadata: &Value) -> Option<String> {
    let entries = metadata.get("managedFields")?.as_array()?;
    let mut managers: Vec<&str> = Vec::new();
    for manager in entries
        .iter()
        .filter_map(|entry| entry.get("manager").and_then(Value::as_str))
        .filter(|manager| !manager.is_empty())
    {
        if !managers.contains(&manager) {
            managers.push(manager);
        }
    }
    if managers.is_empty() {
        return None;
    }
    let visible = managers
        .iter()
        .take(MANAGER_LIMIT)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    Some(if managers.len() > MANAGER_LIMIT {
        format!("{visible} +{} more", managers.len() - MANAGER_LIMIT)
    } else {
        visible
    })
}

/// `ownerReferenceFields`: the controller reference, or the first one.
fn owner_fields(metadata: &Value) -> Vec<(&'static str, String)> {
    let Some(references) = metadata.get("ownerReferences").and_then(Value::as_array) else {
        return Vec::new();
    };
    let owners: Vec<&Value> = references
        .iter()
        .filter(|owner| owner.is_object())
        .collect();
    let Some(owner) = owners
        .iter()
        .copied()
        .find(|owner| owner["controller"] == true)
        .or_else(|| owners.first().copied())
    else {
        return Vec::new();
    };
    [
        ("Controller", "kind"),
        ("Owner", "name"),
        ("Owner UID", "uid"),
    ]
    .into_iter()
    .filter_map(|(label, key)| {
        owner
            .get(key)
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .map(|text| (label, text.to_owned()))
    })
    .collect()
}

/// `gitOpsOwnership(...).details` for the Argo CD and Flux owners the web recognizes.
fn gitops_rows(summary: &ResourceSummary) -> Vec<(&'static str, String)> {
    let Some(owner) = gitops_owner(summary) else {
        return Vec::new();
    };
    let Some(label) = gitops_label(&owner) else {
        return Vec::new();
    };
    let evidence = evidence_label(&owner);
    let evidence = if owner.partial {
        format!("{evidence} (partial)")
    } else {
        evidence
    };
    vec![("GitOps", label), ("Evidence", evidence)]
}

fn gitops_owner(summary: &ResourceSummary) -> Option<GitOpsOwnerSummary> {
    if let Some(owner) = &summary.git_ops_owner {
        return (!owner.name.trim().is_empty()).then(|| owner.clone());
    }
    let application = summary.argo_app.as_deref()?.trim();
    (!application.is_empty()).then(|| GitOpsOwnerSummary {
        provider: "argo".into(),
        kind: "Application".into(),
        name: application.to_owned(),
        namespace: None,
        confidence: "metadata".into(),
        provenance: "argocd-name-annotation".into(),
        partial: true,
    })
}

fn gitops_label(owner: &GitOpsOwnerSummary) -> Option<String> {
    match (owner.provider.as_str(), owner.kind.as_str()) {
        ("argo", "Application") => Some(format!("Owned by Argo CD: {}", owner.name)),
        ("flux", "Kustomization" | "HelmRelease") => {
            let name = owner.namespace.as_ref().map_or_else(
                || owner.name.clone(),
                |namespace| format!("{namespace}/{}", owner.name),
            );
            Some(format!("Owned by Flux {}: {name}", owner.kind))
        }
        _ => None,
    }
}

/// `evidenceLabel` in `src/lib/gitops-ownership-evidence.ts`.
fn evidence_label(owner: &GitOpsOwnerSummary) -> String {
    match owner.provenance.as_str() {
        "argocd-name-annotation" => "Argo CD name annotation".into(),
        "argocd-tracking-annotation" => "Argo CD tracking annotation".into(),
        "argocd-application-label" => "Argo CD Application label".into(),
        "flux-kustomization-labels" => "Flux Kustomization labels".into(),
        "flux-helmrelease-labels" => "Flux HelmRelease labels".into(),
        _ if owner.confidence == "inventory" => "Flux inventory".into(),
        _ => format!("{} metadata", owner.confidence),
    }
}

/// The raw top-level metadata fields, formatted as `formatRawValue` does.
fn raw_rows(metadata: &Value) -> Vec<(String, String)> {
    let Some(map) = metadata.as_object() else {
        return Vec::new();
    };
    map.iter()
        .map(|(key, value)| (key.clone(), raw_text(value)))
        .collect()
}

fn raw_text(value: &Value) -> String {
    match value {
        Value::Null => "-".into(),
        Value::String(text) => text.clone(),
        Value::Bool(_) | Value::Number(_) => value.to_string(),
        _ => serde_json::to_string_pretty(value).unwrap_or_default(),
    }
}

/// A `grid-cols-[7rem_minmax(0,1fr)]` row whose key and value carry test labels.
fn field(key: &str, value: impl IntoElement) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .py(px(6.))
        .child(
            ui::muted(ui::xs(key.to_owned()))
                .w(px(112.))
                .flex_none()
                .id(format!("meta-key:{key}"))
                .test_support()
                .aria_label(key.to_owned()),
        )
        .child(div().flex().flex_1().min_w_0().child(value))
}

/// A bold value whose text is also its test label.
fn labeled_value(key: &str, text: String) -> impl IntoElement {
    ui::semibold(ui::xs(text.clone()))
        .truncate()
        .id(format!("meta-value:{key}"))
        .test_support()
        .aria_label(text)
}
