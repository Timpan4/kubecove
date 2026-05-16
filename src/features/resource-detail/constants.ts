export type Tab = "details" | "events" | "yaml";

export type ChipVariant = "neutral" | "success" | "warning" | "error" | "info";

export const CHIP_BADGE_STYLES: Record<
	ChipVariant,
	{
		variant: "secondary" | "destructive" | "outline";
		className: string;
	}
> = {
	neutral: { variant: "secondary", className: "" },
	success: {
		variant: "outline",
		className:
			"border-emerald-500/30 bg-emerald-500/10 text-emerald-300 dark:bg-emerald-500/15",
	},
	warning: {
		variant: "outline",
		className:
			"border-amber-500/30 bg-amber-500/10 text-amber-300 dark:bg-amber-500/15",
	},
	error: { variant: "destructive", className: "" },
	info: {
		variant: "outline",
		className:
			"border-sky-500/30 bg-sky-500/10 text-sky-300 dark:bg-sky-500/15",
	},
};

export const PANEL_CLASS =
	"flex h-full min-w-0 flex-col overflow-hidden border-l bg-card";
export const PANEL_HEADER_CLASS =
	"flex shrink-0 items-center justify-between border-b px-4 py-3";
export const PANEL_TITLE_CLASS = "truncate whitespace-nowrap text-sm font-semibold";
export const PANEL_TABS_CLASS = "flex shrink-0 border-b";
export const PANEL_TAB_CLASS =
	"rounded-none border-b-2 border-transparent bg-transparent px-4 py-2 text-[13px] text-muted-foreground shadow-none transition-colors hover:text-foreground data-[state=active]:border-primary data-[state=active]:bg-transparent data-[state=active]:text-primary data-[state=active]:shadow-none";
export const PANEL_BODY_CLASS = "flex-1 overflow-y-auto p-4";
export const DETAIL_SECTION_CLASS = "mb-4";
export const DETAIL_SECTION_TITLE_CLASS =
	"mb-2 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground";
export const DETAIL_ROW_CLASS = "flex gap-3 border-b py-1.5";
export const DETAIL_KEY_CLASS = "min-w-[120px] text-xs font-medium text-muted-foreground";
export const DETAIL_VALUE_CLASS = "min-w-0 flex-1 wrap-anywhere text-xs text-foreground";
export const LOADING_STATE_CLASS = "p-6 text-center text-xs text-muted-foreground";
export const ERROR_STATE_CLASS = "p-6 text-center text-xs text-destructive";
export const LOADING_SPINNER_CLASS =
	"mx-auto mb-2 size-4 animate-spin rounded-full border-2 border-muted border-t-primary";
export const YAML_BLOCK_CLASS =
	"whitespace-pre-wrap break-normal font-mono text-xs leading-relaxed text-foreground [overflow-wrap:anywhere]";
export const JSON_BLOCK_CLASS =
	"max-h-[220px] overflow-auto rounded-md border bg-background p-3 font-mono text-xs leading-relaxed whitespace-pre-wrap text-foreground [overflow-wrap:anywhere]";
