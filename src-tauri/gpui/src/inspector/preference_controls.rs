use super::{
    Inspector,
    controls::{Variant, button},
    settings_page::{settings_row, switch},
};
use crate::{
    preferences::*,
    settings::{Control, Row},
    theme::Tokens,
    ui,
};
use gpui_kit::{AnyElement, Context, SharedString, TestSupportExt, div, prelude::*, px};

impl Inspector {
    pub(super) fn preference_control(
        &self,
        row: &'static Row,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = &self.persisted.preferences;
        let checked = match row.title {
            "Show exact timestamps" => Some(p.show_exact_timestamps),
            "Show CPU and memory footer" => Some(p.show_usage_footer),
            "Redact secrets" => Some(p.redact_secrets),
            "Keep full map visible during selection" => Some(p.show_full_topology_on_selection),
            "Show ownership map by default" => Some(p.show_ownership_map_by_default),
            "Expand timeline by default" => Some(p.expand_timeline_by_default),
            "Show unavailable GitOps providers" => Some(p.show_unavailable_git_ops_providers),
            "Show Custom Resources" => Some(p.show_custom_resources),
            "Auto-start saved port forwards" => Some(p.auto_start_saved_port_forwards),
            "Keep live sessions across workspace switches" => {
                Some(p.keep_live_sessions_on_workspace_switch)
            }
            "YAML error lens" => Some(p.yaml_error_lens_enabled),
            "Allow YAML force-conflicts" => Some(p.allow_yaml_force_conflicts),
            "Enable diagnostics" => Some(p.debug_mode_enabled),
            "Skip change review" => Some(p.skip_change_review),
            _ => None,
        };
        let widget = if let Some(checked) = checked {
            switch(row.title, checked)
                .on_click(cx.listener(move |this, _, window, cx| {
                    if row.title == "Skip change review"
                        && !this.persisted.preferences.skip_change_review
                    {
                        this.settings.confirming_skip_review = true;
                        cx.notify();
                        return;
                    }
                    match row.title {
                        "Show exact timestamps" => {
                            this.persisted.preferences.show_exact_timestamps =
                                !this.persisted.preferences.show_exact_timestamps
                        }
                        "Show CPU and memory footer" => {
                            this.persisted.preferences.show_usage_footer =
                                !this.persisted.preferences.show_usage_footer
                        }
                        "Redact secrets" => {
                            this.persisted.preferences.redact_secrets =
                                !this.persisted.preferences.redact_secrets
                        }
                        "Keep full map visible during selection" => {
                            this.persisted.preferences.show_full_topology_on_selection =
                                !this.persisted.preferences.show_full_topology_on_selection
                        }
                        "Show ownership map by default" => {
                            this.persisted.preferences.show_ownership_map_by_default =
                                !this.persisted.preferences.show_ownership_map_by_default
                        }
                        "Expand timeline by default" => {
                            this.persisted.preferences.expand_timeline_by_default =
                                !this.persisted.preferences.expand_timeline_by_default
                        }
                        "Show unavailable GitOps providers" => {
                            this.persisted
                                .preferences
                                .show_unavailable_git_ops_providers = !this
                                .persisted
                                .preferences
                                .show_unavailable_git_ops_providers
                        }
                        "Show Custom Resources" => {
                            this.persisted.preferences.show_custom_resources =
                                !this.persisted.preferences.show_custom_resources
                        }
                        "Auto-start saved port forwards" => {
                            this.persisted.preferences.auto_start_saved_port_forwards =
                                !this.persisted.preferences.auto_start_saved_port_forwards
                        }
                        "Keep live sessions across workspace switches" => {
                            this.persisted
                                .preferences
                                .keep_live_sessions_on_workspace_switch = !this
                                .persisted
                                .preferences
                                .keep_live_sessions_on_workspace_switch
                        }
                        "YAML error lens" => {
                            this.persisted.preferences.yaml_error_lens_enabled =
                                !this.persisted.preferences.yaml_error_lens_enabled
                        }
                        "Allow YAML force-conflicts" => {
                            this.persisted.preferences.allow_yaml_force_conflicts =
                                !this.persisted.preferences.allow_yaml_force_conflicts
                        }
                        "Enable diagnostics" => {
                            this.persisted.preferences.debug_mode_enabled =
                                !this.persisted.preferences.debug_mode_enabled
                        }
                        "Skip change review" => {
                            this.persisted.preferences.skip_change_review =
                                !this.persisted.preferences.skip_change_review
                        }
                        _ => return,
                    }
                    if row.title == "Show CPU and memory footer" {
                        this.toggle_usage(cx);
                    }
                    if row.title == "Show Custom Resources" {
                        this.discover_kinds(cx);
                    }
                    if row.title == "Enable diagnostics" {
                        this.refresh_diagnostics(false, cx);
                        this.timings
                            .set_enabled(this.persisted.preferences.debug_mode_enabled);
                    }
                    if row.title == "Redact secrets" {
                        if let super::detail::Detail::Ready(detail) = &mut this.detail {
                            detail.secrets = Default::default();
                        }
                        this.reload_yaml_view(window, cx);
                    }
                    if row.title == "YAML error lens" {
                        this.refresh_yaml_lens(cx);
                    }
                    this.filter(cx);
                    this.save_preferences(cx);
                }))
                .into_any_element()
        } else if let Control::Segments(options, _) = row.control {
            let selected = match row.title {
                "Timestamp timezone" => match p.timestamp_timezone {
                    TimestampTimezone::Local => 0,
                    TimestampTimezone::Utc => 1,
                },
                "Log layout" => match p.log_layout {
                    LogLayout::Stacked => 0,
                    LogLayout::Gutter => 1,
                    LogLayout::OneLine => 2,
                },
                "YAML shape" => match p.yaml_view_mode_default {
                    YamlShape::Kubectl => 0,
                    YamlShape::ApplyClean => 1,
                },
                "YAML encoding" => match p.yaml_encoding_default {
                    YamlFormat::Yaml => 0,
                    YamlFormat::Kyaml => 1,
                },
                "YAML diff appearance" => match p.yaml_diff_style {
                    DiffStyle::Clean => 0,
                    DiffStyle::Git => 1,
                },
                _ => 0,
            };
            div()
                .flex()
                .items_center()
                .p(px(2.))
                .rounded(px(10.))
                .bg(Tokens::background())
                .border_1()
                .border_color(Tokens::border().opacity(0.6))
                .children(options.iter().enumerate().map(|(index, option)| {
                    let active = index == selected;
                    button(Variant::Ghost, 28., false)
                        .id(SharedString::from(format!(
                            "setting-option:{}:{option}",
                            row.title
                        )))
                        .test_support()
                        .aria_label(*option)
                        .aria_selected(active)
                        .px(px(10.))
                        .rounded(px(8.))
                        .when(active, |b| b.bg(Tokens::card()))
                        .child(ui::medium(ui::xs(*option)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            match row.title {
                                "Timestamp timezone" => {
                                    this.persisted.preferences.timestamp_timezone = match index {
                                        0 => TimestampTimezone::Local,
                                        _ => TimestampTimezone::Utc,
                                    }
                                }
                                "Log layout" => {
                                    this.persisted.preferences.log_layout = match index {
                                        0 => LogLayout::Stacked,
                                        1 => LogLayout::Gutter,
                                        _ => LogLayout::OneLine,
                                    }
                                }
                                "YAML shape" => {
                                    this.persisted.preferences.yaml_view_mode_default = match index
                                    {
                                        0 => YamlShape::Kubectl,
                                        _ => YamlShape::ApplyClean,
                                    }
                                }
                                "YAML encoding" => {
                                    this.persisted.preferences.yaml_encoding_default = match index {
                                        0 => YamlFormat::Yaml,
                                        _ => YamlFormat::Kyaml,
                                    }
                                }
                                "YAML diff appearance" => {
                                    this.persisted.preferences.yaml_diff_style = match index {
                                        0 => DiffStyle::Clean,
                                        _ => DiffStyle::Git,
                                    }
                                }
                                _ => return,
                            }
                            this.save_preferences(cx);
                        }))
                }))
                .into_any_element()
        } else {
            div().into_any_element()
        };
        let row_view = settings_row(row.title, row.description.into(), widget);
        if row.title == "Skip change review" && self.settings.confirming_skip_review {
            div().flex().flex_col().gap(px(12.)).child(row_view)
                .child(ui::xs("A successful dry-run applies changes without showing the diff, even in production. Deletes still require confirmation."))
                .children([("Keep review", false), ("Skip review", true)].into_iter().map(|(label, enable)| {
                    button(Variant::Outline, 28., false).id(SharedString::from(format!("settings-button:{label}"))).test_support().aria_label(label).child(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.settings.confirming_skip_review = false;
                            if enable { this.persisted.preferences.skip_change_review = true; this.save_preferences(cx); }
                            cx.notify();
                        }))
                })).into_any_element()
        } else {
            row_view.into_any_element()
        }
    }
    pub(super) fn resource_order_controls(
        &self,
        row: &'static Row,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let lanes = &self.persisted.preferences.resource_kind_lanes;
        div()
            .id("setting-row:Resource group order")
            .test_support()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(12.))
            .rounded(px(8.))
            .border_1()
            .border_color(Tokens::border())
            .bg(Tokens::card())
            .child(ui::medium(ui::sm(row.title)))
            .child(ui::muted(ui::xs(row.description)))
            .child(
                button(Variant::Outline, 24., false)
                    .id("reset-kind-order")
                    .test_support()
                    .aria_label("Reset")
                    .child("Reset")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.persisted.preferences.resource_kind_lanes = KindLanes::default();
                        this.save_preferences(cx);
                    })),
            )
            .child(
                div().flex().gap(px(8.)).children(
                    [
                        ("Show first", &lanes.first),
                        ("Normal", &lanes.normal),
                        ("Show last", &lanes.last),
                    ]
                    .into_iter()
                    .map(|(title, kinds)| {
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .gap(px(6.))
                            .child(ui::medium(ui::sm(title)))
                            .children(kinds.iter().enumerate().map(|(position, kind)| {
                                let target = kind.clone();
                                div()
                                    .id(SharedString::from(format!("kind-order-item:{kind}")))
                                    .test_support()
                                    .aria_description(format!("{title}:{position}"))
                                    .flex()
                                    .gap(px(4.))
                                    .items_center()
                                    .child(
                                        ui::sm(if kind == "*" {
                                            "All other kinds".to_owned()
                                        } else {
                                            kind.clone()
                                        })
                                        .flex_1(),
                                    )
                                    .children(
                                        [("Earlier", true), ("Later", false)].into_iter().map(
                                            |(label, earlier)| {
                                                let target = target.clone();
                                                button(Variant::Outline, 24., false)
                                                    .id(SharedString::from(format!(
                                                        "kind-order:{target}:{label}"
                                                    )))
                                                    .test_support()
                                                    .aria_label(format!("Move {target} {label}"))
                                                    .child(if earlier { "←" } else { "→" })
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        let lanes = &mut this
                                                            .persisted
                                                            .preferences
                                                            .resource_kind_lanes;
                                                        let all = [
                                                            &mut lanes.first,
                                                            &mut lanes.normal,
                                                            &mut lanes.last,
                                                        ];
                                                        if let Some(lane) =
                                                            all.iter().position(|items| {
                                                                items.contains(&target)
                                                            })
                                                        {
                                                            let index = all[lane]
                                                                .iter()
                                                                .position(|k| k == &target)
                                                                .unwrap();
                                                            if earlier && index > 0 {
                                                                all[lane].swap(index, index - 1);
                                                            } else if !earlier
                                                                && index + 1 < all[lane].len()
                                                            {
                                                                all[lane].swap(index, index + 1);
                                                            } else if earlier && lane > 0 {
                                                                let k = all[lane].remove(index);
                                                                all[lane - 1].push(k);
                                                            } else if !earlier
                                                                && lane + 1 < all.len()
                                                            {
                                                                let k = all[lane].remove(index);
                                                                all[lane + 1].insert(0, k);
                                                            }
                                                        }
                                                        this.save_preferences(cx);
                                                    }))
                                            },
                                        ),
                                    )
                            }))
                    }),
                ),
            )
            .into_any_element()
    }
}
