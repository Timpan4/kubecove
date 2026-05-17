import type { ReactNode } from "react";
import {
	ResizableHandle,
	ResizablePanel,
	ResizablePanelGroup,
} from "@/components/ui/resizable";

const MAIN_PANEL_DEFAULT_SIZE = 60;
const DETAIL_PANEL_DEFAULT_SIZE = 40;
const MAIN_PANEL_MIN_SIZE = 30;
const DETAIL_PANEL_MIN_SIZE = 33;

export function DetailPanelFrame({
	mainContent,
	detailPanel,
}: {
	mainContent: ReactNode;
	detailPanel: ReactNode;
}) {
	if (!detailPanel) return mainContent;

	return (
		<ResizablePanelGroup
			orientation="horizontal"
			className="min-w-0 flex-1 overflow-hidden"
		>
			<ResizablePanel
				defaultSize={MAIN_PANEL_DEFAULT_SIZE}
				minSize={MAIN_PANEL_MIN_SIZE}
				className="min-w-0 overflow-hidden"
			>
				{mainContent}
			</ResizablePanel>
			<ResizableHandle
				withHandle
				aria-label="Resize details panel"
				className="w-2 bg-transparent data-[resize-handle-state=hover]:bg-border/40 data-[resize-handle-state=drag]:bg-border/60"
			/>
			<ResizablePanel
				defaultSize={DETAIL_PANEL_DEFAULT_SIZE}
				minSize={DETAIL_PANEL_MIN_SIZE}
				className="h-full min-w-0 overflow-hidden"
			>
				{detailPanel}
			</ResizablePanel>
		</ResizablePanelGroup>
	);
}
