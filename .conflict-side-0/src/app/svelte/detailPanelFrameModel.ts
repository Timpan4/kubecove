export const MAIN_PANEL_DEFAULT_SIZE = 60;
export const DETAIL_PANEL_DEFAULT_SIZE = 40;
export const MAIN_PANEL_MIN_SIZE = 30;
export const DETAIL_PANEL_MIN_SIZE = 33;
export const DETAIL_PANEL_RESIZE_STEP = 2;

export interface HorizontalResizeRect {
	width: number;
	right: number;
}

export function clampDetailPanelSize(
	size: number,
	minSize = DETAIL_PANEL_MIN_SIZE,
	mainMinSize = MAIN_PANEL_MIN_SIZE,
): number {
	const maxDetailSize = 100 - mainMinSize;
	return Math.min(maxDetailSize, Math.max(minSize, size));
}

export function detailPanelSizeFromPointer(
	rect: HorizontalResizeRect,
	clientX: number,
	defaultSize = DETAIL_PANEL_DEFAULT_SIZE,
	minSize = DETAIL_PANEL_MIN_SIZE,
	mainMinSize = MAIN_PANEL_MIN_SIZE,
): number {
	if (rect.width <= 0) return defaultSize;
	return clampDetailPanelSize(((rect.right - clientX) / rect.width) * 100, minSize, mainMinSize);
}

export function detailPanelSizeFromKey(
	currentSize: number,
	key: string,
	minSize = DETAIL_PANEL_MIN_SIZE,
	mainMinSize = MAIN_PANEL_MIN_SIZE,
): number | null {
	if (key === "ArrowLeft") {
		return clampDetailPanelSize(currentSize + DETAIL_PANEL_RESIZE_STEP, minSize, mainMinSize);
	}
	if (key === "ArrowRight") {
		return clampDetailPanelSize(currentSize - DETAIL_PANEL_RESIZE_STEP, minSize, mainMinSize);
	}
	if (key === "Home") return minSize;
	if (key === "End") return 100 - mainMinSize;
	return null;
}
