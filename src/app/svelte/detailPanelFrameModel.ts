export const MAIN_PANEL_DEFAULT_SIZE = 60;
export const DETAIL_PANEL_DEFAULT_SIZE = 40;
export const MAIN_PANEL_MIN_SIZE = 30;
export const DETAIL_PANEL_MIN_SIZE = 33;
export const DETAIL_PANEL_RESIZE_STEP = 2;

export interface HorizontalResizeRect {
	width: number;
	right: number;
}

export function clampDetailPanelSize(size: number): number {
	const maxDetailSize = 100 - MAIN_PANEL_MIN_SIZE;
	return Math.min(maxDetailSize, Math.max(DETAIL_PANEL_MIN_SIZE, size));
}

export function detailPanelSizeFromPointer(
	rect: HorizontalResizeRect,
	clientX: number,
): number {
	if (rect.width <= 0) return DETAIL_PANEL_DEFAULT_SIZE;
	return clampDetailPanelSize(((rect.right - clientX) / rect.width) * 100);
}

export function detailPanelSizeFromKey(currentSize: number, key: string): number | null {
	if (key === "ArrowLeft") return clampDetailPanelSize(currentSize + DETAIL_PANEL_RESIZE_STEP);
	if (key === "ArrowRight") return clampDetailPanelSize(currentSize - DETAIL_PANEL_RESIZE_STEP);
	if (key === "Home") return DETAIL_PANEL_MIN_SIZE;
	if (key === "End") return 100 - MAIN_PANEL_MIN_SIZE;
	return null;
}
