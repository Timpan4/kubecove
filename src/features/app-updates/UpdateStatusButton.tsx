import { useMemo, useState } from "react";
import {
	AlertTriangle,
	CheckCircle2,
	Download,
	RefreshCw,
	RotateCw,
	X,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	Popover,
	PopoverContent,
	PopoverTrigger,
} from "@/components/ui/popover";
import {
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import { isAppUpdatesEnabled } from "@/lib/release-channel";
import { cn } from "@/lib/utils";
import { useAppUpdateStore } from "./store";

const CHECKED_AT_FORMATTER = new Intl.DateTimeFormat(undefined, {
	dateStyle: "medium",
	timeStyle: "short",
});

function formatCheckedAt(value: string | null): string {
	if (!value) return "Not checked yet";
	const date = new Date(value);
	if (Number.isNaN(date.getTime())) return "Not checked yet";
	return CHECKED_AT_FORMATTER.format(date);
}

export function UpdateStatusButton() {
	const updatesEnabled = isAppUpdatesEnabled();
	const {
		status,
		currentVersion,
		availableVersion,
		releaseNotes,
		downloadProgress,
		lastCheckedAt,
		errorMessage,
		dismissedVersion,
		checkForUpdates,
		installUpdate,
		relaunchApp,
		dismissUpdate,
	} = useAppUpdateStore();
	const [manualOpen, setManualOpen] = useState(false);
	const [autoOpenedVersion, setAutoOpenedVersion] = useState<string | null>(null);
	const hasUpdate = status === "available" || status === "downloading";
	const shouldAutoOpen =
		updatesEnabled &&
		status === "available" &&
		availableVersion !== null &&
		dismissedVersion !== availableVersion;
	const autoOpenVersion = shouldAutoOpen ? availableVersion : null;
	const open =
		manualOpen ||
		(autoOpenVersion !== null && autoOpenedVersion !== autoOpenVersion);
	const setOpen = (nextOpen: boolean) => {
		setManualOpen(nextOpen);
		if (!nextOpen && autoOpenVersion !== null) {
			setAutoOpenedVersion(autoOpenVersion);
		}
	};

	const tooltip = useMemo(() => {
		if (status === "available") return "Update available";
		if (status === "downloading") return "Downloading update";
		if (status === "installed") return "Restart to finish update";
		if (status === "error") return "Update check failed";
		if (status === "checking") return "Checking for updates";
		return "Check for updates";
	}, [status]);

	if (!updatesEnabled) return null;

	return (
		<Popover open={open} onOpenChange={setOpen}>
			<Tooltip>
				<TooltipTrigger asChild>
					<PopoverTrigger asChild>
						<Button
							type="button"
							variant="ghost"
							size="icon"
							className="relative mr-1 size-8 text-muted-foreground [-webkit-app-region:no-drag]"
							aria-label={tooltip}
						>
							<RotateCw
								className={cn(status === "checking" && "animate-spin")}
							/>
							{(hasUpdate || status === "installed") && (
								<span className="absolute right-1 top-1 size-2 rounded-full bg-primary" />
							)}
						</Button>
					</PopoverTrigger>
				</TooltipTrigger>
				<TooltipContent>{tooltip}</TooltipContent>
			</Tooltip>
			<PopoverContent align="end" className="flex flex-col gap-3">
				<div className="flex items-start justify-between gap-3">
					<div className="min-w-0">
						<div className="text-sm font-semibold text-foreground">
							App updates
						</div>
						<div className="mt-1 text-xs text-muted-foreground">
							Current version {currentVersion}
						</div>
					</div>
					{availableVersion && <Badge variant="secondary">{availableVersion}</Badge>}
				</div>

				{status === "available" && (
					<div className="flex flex-col gap-3">
						<div className="text-sm text-foreground">
							KubeCove {availableVersion} is available.
						</div>
						{releaseNotes && (
							<div className="max-h-28 overflow-y-auto rounded-sm border bg-muted/40 p-2 text-xs leading-relaxed text-muted-foreground">
								{releaseNotes}
							</div>
						)}
						<div className="flex justify-end gap-2">
							<Button
								type="button"
								variant="ghost"
								onClick={() => {
									if (availableVersion) dismissUpdate(availableVersion);
									setOpen(false);
								}}
							>
								<X data-icon="inline-start" />
								Later
							</Button>
							<Button type="button" onClick={() => void installUpdate()}>
								<Download data-icon="inline-start" />
								Install
							</Button>
						</div>
					</div>
				)}

				{status === "downloading" && (
					<div className="flex flex-col gap-2">
						<div className="text-sm text-foreground">Downloading update...</div>
						<div className="h-2 overflow-hidden rounded-full bg-muted">
							<div
								className="h-full bg-primary transition-all"
								style={{ width: `${downloadProgress ?? 12}%` }}
							/>
						</div>
						<div className="text-xs text-muted-foreground">
							{downloadProgress === null
								? "Preparing installer"
								: `${downloadProgress}%`}
						</div>
					</div>
				)}

				{status === "installed" && (
					<div className="flex flex-col gap-3">
						<div className="flex items-center gap-2 text-sm text-foreground">
							<CheckCircle2 data-icon="inline-start" />
							Update installed. Restart KubeCove to finish.
						</div>
						<Button type="button" onClick={() => void relaunchApp()}>
							<RefreshCw data-icon="inline-start" />
							Relaunch
						</Button>
					</div>
				)}

				{status === "upToDate" && (
					<div className="flex items-center gap-2 text-sm text-muted-foreground">
						<CheckCircle2 data-icon="inline-start" />
						KubeCove is up to date.
					</div>
				)}

				{status === "error" && (
					<div className="flex flex-col gap-2">
						<div className="flex items-center gap-2 text-sm text-destructive">
							<AlertTriangle data-icon="inline-start" />
							Update check failed.
						</div>
						{errorMessage && (
							<div className="text-xs leading-relaxed text-muted-foreground">
								{errorMessage}
							</div>
						)}
					</div>
				)}

				{(status === "idle" || status === "checking") && (
					<div className="text-sm text-muted-foreground">
						{status === "checking" ? "Checking for updates..." : "Ready to check."}
					</div>
				)}

				<div className="flex items-center justify-between gap-3 border-t pt-3">
					<div className="text-xs text-muted-foreground">
						Last checked: {formatCheckedAt(lastCheckedAt)}
					</div>
					<Button
						type="button"
						variant="outline"
						size="sm"
						disabled={status === "checking" || status === "downloading"}
						onClick={() => void checkForUpdates({ manual: true })}
					>
						<RefreshCw data-icon="inline-start" />
						Check
					</Button>
				</div>
			</PopoverContent>
		</Popover>
	);
}
