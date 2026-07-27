export const PAGE_SIZE = 50;

// Sticky offsets for the group rows; the values are CSS variables measured at
// runtime by ResourceTable (header height, header + app-group height).
export const STICKY_APP_GROUP_TOP = "top-[var(--sticky-app-top,40px)]";
export const STICKY_TYPE_GROUP_TOP = "top-[var(--sticky-type-top,72px)]";

export const TABLE_CLASS =
	"table-fixed border-collapse text-sm [&_th]:sticky [&_th]:top-0 [&_th]:z-30 [&_th]:bg-card [&_th]:shadow-[inset_0_-2px_0_0_var(--border)] [&_th]:px-2.5 [&_th]:py-3 [&_th]:text-left [&_th]:text-xs [&_th]:font-semibold [&_th]:uppercase [&_th]:text-muted-foreground [&_td]:whitespace-nowrap [&_td]:border-b [&_td]:px-2.5 [&_td]:py-3";

export const ROW_CLASS = "cursor-pointer transition-colors hover:bg-accent/60";
export const SELECTED_ROW_CLASS = "bg-accent";
export const EMPTY_PAGE_CLASS = "p-8 text-center text-sm text-muted-foreground";
