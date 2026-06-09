import { Download, RefreshCw } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useAppUpdateStore } from "@/features/app-updates";
import { isAppUpdatesEnabled } from "@/lib/release-channel";
import { SettingsRow, SettingsSection } from "./SettingsControls";

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

export function AppUpdatesSection() {
	const {
		status,
		currentVersion,
		availableVersion,
		downloadProgress,
		lastCheckedAt,
		errorMessage,
		checkForUpdates,
		installUpdate,
		relaunchApp,
	} = useAppUpdateStore();
	const updatesEnabled = isAppUpdatesEnabled();
	const updateBusy = status === "checking" || status === "downloading";

	return (
		<SettingsSection title="Updates">
			<SettingsRow
				title="KubeCove version"
				description={
					updatesEnabled
						? `Current version ${currentVersion}. Last checked: ${formatCheckedAt(lastCheckedAt)}.`
						: `Current version ${currentVersion}. Development build; update checks disabled.`
				}
			>
				<div className="flex items-center gap-2">
					{updatesEnabled && availableVersion && (
						<Badge variant="secondary">Update {availableVersion}</Badge>
					)}
					{updatesEnabled ? (
						<Button
							type="button"
							variant="outline"
							size="sm"
							disabled={updateBusy}
							onClick={() => void checkForUpdates({ manual: true })}
						>
							<RefreshCw data-icon="inline-start" />
							Check
						</Button>
					) : (
						<Badge variant="outline">Dev build</Badge>
					)}
				</div>
			</SettingsRow>
			{updatesEnabled && status === "available" && (
				<SettingsRow
					title="Update available"
					description={`KubeCove ${availableVersion} can be downloaded and installed now.`}
				>
					<Button type="button" size="sm" onClick={() => void installUpdate()}>
						<Download data-icon="inline-start" />
						Install
					</Button>
				</SettingsRow>
			)}
			{updatesEnabled && status === "downloading" && (
				<SettingsRow
					title="Installing update"
					description={
						downloadProgress === null
							? "Preparing the installer."
							: `Download progress: ${downloadProgress}%.`
					}
				>
					<Badge variant="secondary">Working</Badge>
				</SettingsRow>
			)}
			{updatesEnabled && status === "installed" && (
				<SettingsRow
					title="Relaunch required"
					description="The update is installed. Relaunch KubeCove to finish."
				>
					<Button type="button" size="sm" onClick={() => void relaunchApp()}>
						<RefreshCw data-icon="inline-start" />
						Relaunch
					</Button>
				</SettingsRow>
			)}
			{updatesEnabled && status === "error" && errorMessage && (
				<SettingsRow title="Update check failed" description={errorMessage}>
					<Button
						type="button"
						variant="outline"
						size="sm"
						onClick={() => void checkForUpdates({ manual: true })}
					>
						<RefreshCw data-icon="inline-start" />
						Retry
					</Button>
				</SettingsRow>
			)}
		</SettingsSection>
	);
}
