import { useEffect, useState } from "react";
import { takeUiRuntimeReloadNotice } from "@/lib/ui-runtime";

export function RuntimeModeToast() {
	const [message, setMessage] = useState<string | null>(null);

	useEffect(() => {
		const notice = takeUiRuntimeReloadNotice();
		if (!notice) return;
		setMessage(notice);
		const timeout = window.setTimeout(() => setMessage(null), 3_000);
		return () => window.clearTimeout(timeout);
	}, []);

	if (!message) return null;

	return (
		<div
			role="status"
			className="fixed bottom-4 left-1/2 z-50 -translate-x-1/2 rounded-md border bg-popover px-3 py-2 text-sm text-popover-foreground shadow-lg"
		>
			{message}
		</div>
	);
}
