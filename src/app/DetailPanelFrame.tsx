import { useCallback, useState, type ReactNode } from "react";

const DETAIL_PANEL_DEFAULT_WIDTH = 480;
const DETAIL_PANEL_MIN_WIDTH = 390;
const MAIN_PANEL_MIN_WIDTH = 360;
const SIDEBAR_WIDTH = 260;

function clampDetailPanelWidth(width: number): number {
	const viewportWidth =
		typeof window === "undefined" ? 1440 : window.innerWidth;
	const maxWidth = Math.max(
		DETAIL_PANEL_MIN_WIDTH,
		viewportWidth - SIDEBAR_WIDTH - MAIN_PANEL_MIN_WIDTH,
	);
	return Math.min(Math.max(width, DETAIL_PANEL_MIN_WIDTH), maxWidth);
}

export function DetailPanelFrame({
	mainContent,
	detailPanel,
}: {
	mainContent: ReactNode;
	detailPanel: ReactNode;
}) {
	const [detailPanelWidth, setDetailPanelWidth] = useState(
		DETAIL_PANEL_DEFAULT_WIDTH,
	);

	const handleDetailResizeStart = useCallback(
		(event: React.PointerEvent<HTMLDivElement>) => {
			event.preventDefault();
			const startX = event.clientX;
			const startWidth = detailPanelWidth;

			const handlePointerMove = (moveEvent: PointerEvent) => {
				const nextWidth = startWidth + startX - moveEvent.clientX;
				setDetailPanelWidth(clampDetailPanelWidth(nextWidth));
			};

			const handlePointerUp = () => {
				document.body.style.cursor = "";
				document.body.style.userSelect = "";
				window.removeEventListener("pointermove", handlePointerMove);
				window.removeEventListener("pointerup", handlePointerUp);
			};

			document.body.style.cursor = "col-resize";
			document.body.style.userSelect = "none";
			window.addEventListener("pointermove", handlePointerMove);
			window.addEventListener("pointerup", handlePointerUp, { once: true });
		},
		[detailPanelWidth],
	);

	if (!detailPanel) return mainContent;

	return (
		<div className="flex min-w-0 flex-1 overflow-hidden">
			<div className="min-w-0 flex-1 overflow-hidden">{mainContent}</div>
			<div
				role="separator"
				aria-orientation="vertical"
				aria-label="Resize details panel"
				className="group relative flex w-2 shrink-0 cursor-col-resize items-center justify-center"
				onPointerDown={handleDetailResizeStart}
				onDoubleClick={() => setDetailPanelWidth(DETAIL_PANEL_DEFAULT_WIDTH)}
			>
				<div className="h-full w-px bg-border transition-colors group-hover:bg-ring" />
				<div className="absolute h-8 w-1 rounded-full bg-border transition-colors group-hover:bg-ring" />
			</div>
			<div
				className="h-full shrink-0 overflow-hidden"
				style={{
					width: clampDetailPanelWidth(detailPanelWidth),
					minWidth: DETAIL_PANEL_MIN_WIDTH,
				}}
			>
				{detailPanel}
			</div>
		</div>
	);
}
