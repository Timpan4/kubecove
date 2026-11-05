//! The Settings catalog from `src/app/svelte/SettingsSurface.svelte` and its
//! section components: categories, rows and search. Each row records whether
//! the native host can back it and, if not, why.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    General,
    Appearance,
    Sessions,
    Yaml,
    Kubeconfig,
    Updates,
    Diagnostics,
    Advanced,
    Remote,
    Agents,
}

impl Category {
    /// Desktop categories in the web's order, with their lucide icon.
    pub const ALL: [(Self, &'static str, &'static str); 10] = [
        (Self::General, "General", "sliders-horizontal"),
        (Self::Appearance, "Appearance", "palette"),
        (Self::Sessions, "Live sessions", "cable"),
        (Self::Yaml, "YAML", "file-code-corner"),
        (Self::Kubeconfig, "Kubeconfig", "folder-cog"),
        (Self::Updates, "Updates", "refresh-cw"),
        (Self::Diagnostics, "Diagnostics", "bug"),
        (Self::Advanced, "Advanced", "wrench"),
        (Self::Remote, "Remote server", "cable"),
        (Self::Agents, "AI agents", "bot"),
    ];

    pub fn label(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(category, ..)| *category == self)
            .map_or("Settings", |(_, label, _)| label)
    }
}

/// The control a row shows, so the page renders the web's widget shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Switch(bool),
    Segments(&'static [&'static str], usize),
    Button(&'static str),
    /// A custom section the page renders itself.
    Section,
}

/// Why a row is disabled in the native client.
const BROWSER_TOOL: &str =
    "Measures the web frontend's DOM. It does not apply to the native client.";

pub struct Row {
    pub category: Category,
    pub title: &'static str,
    pub description: &'static str,
    pub control: Control,
    /// `None` when the native host backs the row with typed commands.
    pub unavailable: Option<&'static str>,
}

const fn row(
    category: Category,
    title: &'static str,
    description: &'static str,
    control: Control,
    unavailable: Option<&'static str>,
) -> Row {
    Row {
        category,
        title,
        description,
        control,
        unavailable,
    }
}

use Category as C;
use Control::{Button, Section, Segments, Switch};

/// Every searchable row. Defaults mirror `src/lib/settings.ts`.
pub const ROWS: &[Row] = &[
    row(
        C::General,
        "KubeCove documentation",
        "Opens user guides and reference documentation in your browser.",
        Button("Open Wiki"),
        None,
    ),
    row(
        C::General,
        "Show exact timestamps",
        "Adds exact timestamps next to relative ages.",
        Switch(false),
        None,
    ),
    row(
        C::General,
        "Timestamp timezone",
        "Controls exact timestamps in labels and tooltips.",
        Segments(&["Local", "UTC"], 0),
        None,
    ),
    row(
        C::General,
        "Show CPU and memory footer",
        "Shows compact app process CPU and memory usage.",
        Switch(false),
        None,
    ),
    row(
        C::General,
        "Redact secrets",
        "Keeps Secret values hidden by default. Connected Argo Secret values stay provider masked.",
        Switch(true),
        None,
    ),
    row(
        C::General,
        "Keep full map visible during selection",
        "Shows unrelated ownership branches while a resource is selected. Large namespaces may render slower.",
        Switch(false),
        None,
    ),
    row(
        C::General,
        "Show ownership map by default",
        "Opens the ownership map in new resource views. Returning to a view restores its previous layout.",
        Switch(true),
        None,
    ),
    row(
        C::General,
        "Expand timeline by default",
        "Shows messages and facts in resource timelines. You can still collapse individual entries.",
        Switch(false),
        None,
    ),
    row(
        C::General,
        "Show unavailable GitOps providers",
        "Shows Argo CD and Flux provider groups as disabled when CRDs are missing.",
        Switch(false),
        None,
    ),
    row(
        C::General,
        "Show Custom Resources",
        "Loads Custom Resources after native Kubernetes resources are visible.",
        Switch(true),
        None,
    ),
    row(
        C::Appearance,
        "Log layout",
        "Controls how entries are shown in the Logs tab.",
        Segments(&["Stacked", "Gutter", "One line"], 0),
        None,
    ),
    row(
        C::Appearance,
        "Resource group order",
        "Move kinds earlier or later to order Resource Table groups. Column sorting still orders rows inside each group.",
        Section,
        None,
    ),
    row(
        C::Sessions,
        "Auto-start saved port forwards",
        "Starts saved Service port-forward presets when a workspace is restored.",
        Switch(false),
        None,
    ),
    row(
        C::Sessions,
        "Keep live sessions across workspace switches",
        "Leaves port-forward and Pod exec sessions running when you leave their scope.",
        Switch(false),
        None,
    ),
    row(
        C::Yaml,
        "YAML shape",
        "Controls whether YAML opens as Kubectl view or Apply clean output.",
        Segments(&["Kubectl view", "Apply clean"], 0),
        None,
    ),
    row(
        C::Yaml,
        "YAML encoding",
        "Controls whether YAML panels open as regular YAML or KYAML.",
        Segments(&["YAML", "KYAML"], 0),
        None,
    ),
    row(
        C::Yaml,
        "YAML diff appearance",
        "Controls selected-resource dry-run diff rendering.",
        Segments(&["Clean", "Git"], 0),
        None,
    ),
    row(
        C::Yaml,
        "YAML error lens",
        "Shows inline editor diagnostics below YAML lines.",
        Switch(true),
        None,
    ),
    row(
        C::Yaml,
        "Allow YAML force-conflicts",
        "Lets guarded YAML operations take server-side field ownership.",
        Switch(false),
        None,
    ),
    row(
        C::Kubeconfig,
        "Active sources",
        "Shows where contexts are discovered from. Full file paths are shown only here.",
        Section,
        None,
    ),
    // The backend reports whether these two writes persist.
    row(
        C::Kubeconfig,
        "Environment variable",
        "Variable holding kubeconfig paths. The default path is read when it is unset.",
        Section,
        None,
    ),
    row(
        C::Kubeconfig,
        "Show source labels",
        "Marks contexts with the kubeconfig source they were discovered from.",
        Section,
        None,
    ),
    row(
        C::Kubeconfig,
        "Added kubeconfig paths",
        "Extra kubeconfig files KubeCove reads alongside the environment variable.",
        Section,
        None,
    ),
    row(
        C::Updates,
        "KubeCove version",
        "Current version, update checks, and installing new releases.",
        Section,
        None,
    ),
    row(
        C::Diagnostics,
        "Enable diagnostics",
        "Collects local latency timings for release smoke testing. Trace data stays in memory.",
        Switch(false),
        None,
    ),
    row(
        C::Diagnostics,
        "Record interactions",
        "Measures input-to-paint time, dropped frames, and DOM changes for each click, drag, key, and scroll.",
        Switch(false),
        Some(BROWSER_TOOL),
    ),
    row(
        C::Diagnostics,
        "Interaction report",
        "Interactions with dropped frames, worst first. Enable diagnostics to include Tauri command timings. Copy outputs JSON lines for bun run perf:interactions.",
        Section,
        Some(BROWSER_TOOL),
    ),
    #[cfg(debug_assertions)]
    row(
        C::Diagnostics,
        "Topology spike",
        "Opens the Svelte topology benchmark harness with synthetic 4,000-node data.",
        Button("Open 4k LOD"),
        Some(BROWSER_TOOL),
    ),
    row(
        C::Diagnostics,
        "Latency report",
        "Shows frontend and backend timing summaries, with redacted copy output by default.",
        Section,
        None,
    ),
    row(
        C::Advanced,
        "Skip change review",
        "Successful dry-runs apply without showing a diff, across every context including production. Dry-run remains required, and deletes always ask.",
        Switch(false),
        None,
    ),
    row(
        C::Remote,
        "Remote KubeCove server",
        "Connect this desktop to a headless KubeCove host.",
        Section,
        None,
    ),
    row(
        C::Agents,
        "MCP server",
        "Lets AI agents such as Claude Code read cluster state through read-only tools on 127.0.0.1. Off by default.",
        Switch(false),
        None,
    ),
    row(
        C::Agents,
        "MCP port",
        "Loopback port for the MCP endpoint. Agents connect to http://127.0.0.1:<port>/mcp.",
        Section,
        None,
    ),
    row(
        C::Agents,
        "Agent access token",
        "Agents send this bearer token. Rotating it disconnects every agent configured with the old one.",
        Section,
        None,
    ),
];

/// `matchesSettingsQuery`.
pub fn matches(query: &str, row: &Row) -> bool {
    let needle = query.trim().to_lowercase();
    needle.is_empty()
        || row.title.to_lowercase().contains(&needle)
        || row.description.to_lowercase().contains(&needle)
}

/// `showCategory`: the active category, or while searching every category
/// with a matching row. The web then shows the whole category.
pub fn shown(query: &str, active: Category, category: Category) -> bool {
    if query.trim().is_empty() {
        return category == active;
    }
    ROWS.iter()
        .any(|row| row.category == category && matches(query, row))
}

/// `hasMatches`.
pub fn has_matches(query: &str) -> bool {
    ROWS.iter().any(|row| matches(query, row))
}

pub fn rows(category: Category) -> impl Iterator<Item = &'static Row> {
    ROWS.iter().filter(move |row| row.category == category)
}
