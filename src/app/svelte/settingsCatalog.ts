import { IS_DEV_BUILD } from "@/lib/build-env";

export type SettingsRowMeta = { title: string; description: string };

export const GENERAL_ROWS = {
	documentation: {
		title: "KubeCove documentation",
		description: "Opens user guides and reference documentation in your browser.",
	},
	exactTimestamps: {
		title: "Show exact timestamps",
		description: "Adds exact timestamps next to relative ages.",
	},
	timezone: {
		title: "Timestamp timezone",
		description: "Controls exact timestamps in labels and tooltips.",
	},
	usageFooter: {
		title: "Show CPU and memory footer",
		description: "Shows compact app process CPU and memory usage.",
	},
	workingSet: {
		title: "Measure working set (WSS)",
		description:
			"Linux only. Each footer refresh resets the kernel's referenced-page bits for KubeCove processes, so WSS shows memory touched since the previous refresh. The kernel may then reclaim those pages sooner. Windows always reports its working set; macOS has none.",
	},
	fullTopologyOnSelection: {
		title: "Keep full map visible during selection",
		description:
			"Shows unrelated ownership branches while a resource is selected. Large namespaces may render slower.",
	},
	ownershipMap: {
		title: "Show ownership map by default",
		description: "Opens the ownership map in new resource views. Returning to a view restores its previous layout.",
	},
	timelineExpansion: {
		title: "Expand timeline by default",
		description: "Shows messages and facts in resource timelines. You can still collapse individual entries.",
	},
	unavailableGitOpsProviders: {
		title: "Show unavailable GitOps providers",
		description:
			"Shows Argo CD and Flux provider groups as disabled when CRDs are missing.",
	},
	customResources: {
		title: "Show Custom Resources",
		description:
			"Loads Custom Resources after native Kubernetes resources are visible.",
	},
	redactSecrets: {
		title: "Redact secrets",
		description: "Keeps Secret values hidden by default. Connected Argo Secret values stay provider masked.",
	},
} satisfies Record<string, SettingsRowMeta>;

export const SESSION_ROWS = {
	autoStartPortForwards: {
		title: "Auto-start saved port forwards",
		description:
			"Starts saved Service port-forward presets when a workspace is restored.",
	},
	keepLiveSessions: {
		title: "Keep live sessions across workspace switches",
		description:
			"Leaves port-forward and Pod exec sessions running when you leave their scope.",
	},
} satisfies Record<string, SettingsRowMeta>;

export const APPEARANCE_ROWS = {
	logLayout: {
		title: "Log layout",
		description: "Controls how entries are shown in the Logs tab.",
	},
	resourceKindOrder: {
		title: "Resource group order",
		description:
			"Drag kinds between lanes to order Resource Table groups, or focus a kind and use the arrow keys. Column sorting still orders rows inside each group.",
	},
} satisfies Record<string, SettingsRowMeta>;

export const YAML_ROWS = {
	viewMode: {
		title: "YAML shape",
		description:
			"Controls whether YAML opens as Kubectl view or Apply clean output.",
	},
	encoding: {
		title: "YAML encoding",
		description: "Controls whether YAML panels open as regular YAML or KYAML.",
	},
	diffAppearance: {
		title: "YAML diff appearance",
		description: "Controls selected-resource dry-run diff rendering.",
	},
	errorLens: {
		title: "YAML error lens",
		description: "Shows inline editor diagnostics below YAML lines.",
	},
	forceConflicts: {
		title: "Allow YAML force-conflicts",
		description: "Lets guarded YAML operations take server-side field ownership.",
	},
} satisfies Record<string, SettingsRowMeta>;

export const KUBECONFIG_ROWS = {
	activeSources: {
		title: "Active sources",
		description:
			"Shows where contexts are discovered from. Full file paths are shown only here.",
	},
	envVar: {
		title: "Environment variable",
		description:
			"Variable holding kubeconfig paths. The default path is read when it is unset.",
	},
	sourceLabels: {
		title: "Show source labels",
		description:
			"Marks contexts with the kubeconfig source they were discovered from.",
	},
	addedPaths: {
		title: "Added kubeconfig paths",
		description:
			"Extra kubeconfig files KubeCove reads alongside the environment variable.",
	},
} satisfies Record<string, SettingsRowMeta>;

export const UPDATES_ROWS = {
	version: {
		title: "KubeCove version",
		description: "Current version, update checks, and installing new releases.",
	},
} satisfies Record<string, SettingsRowMeta>;

export const MCP_ROWS = {
	server: {
		title: "MCP server",
		description:
			"Lets AI agents such as Claude Code read cluster state through read-only tools on 127.0.0.1. Off by default.",
	},
	port: {
		title: "MCP port",
		description: "Loopback port for the MCP endpoint. Agents connect to http://127.0.0.1:<port>/mcp.",
	},
	access: {
		title: "Agent access token",
		description:
			"Agents send this bearer token. Rotating it disconnects every agent configured with the old one.",
	},
} satisfies Record<string, SettingsRowMeta>;

export const ADVANCED_ROWS = {
	skipChangeReview: {
		title: "Skip change review",
		description:
			"Successful dry-runs apply without showing a diff, across every context including production. Dry-run remains required, and deletes always ask.",
	},
} satisfies Record<string, SettingsRowMeta>;

export const SERVER_INFO_ROW: SettingsRowMeta = {
	title: "Server information",
	description: "KubeCove server version and active credential storage on this host.",
};

export const DIAGNOSTICS_ROWS: SettingsRowMeta[] = [
	{
		title: "Enable diagnostics",
		description:
			"Collects local latency timings for release smoke testing. Trace data stays in memory.",
	},
	{
		title: "Record interactions",
		description:
			"Measures input-to-paint time, dropped frames, and DOM changes for each click, drag, key, and scroll.",
	},
	{
		title: "Latency report",
		description:
			"Shows frontend and backend timing summaries, with redacted copy output by default.",
	},
];

if (IS_DEV_BUILD) {
	const topologySpike: SettingsRowMeta = {
		title: "Topology spike",
		description:
			"Opens the Svelte topology benchmark harness with synthetic 4,000-node data, compact nodes, selected edges, and a focused viewport.",
	};
	DIAGNOSTICS_ROWS.push(topologySpike);
}
