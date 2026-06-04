import type { ReactNode } from "react";
import { Check, Download, RefreshCw, RotateCcw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { ToggleButton } from "@/components/ToggleButton";
import { useAppUpdateStore } from "@/features/app-updates";
import {
	DEFAULT_KUBECONFIG_ENV_VAR,
	normalizeKubeconfigEnvVar,
	type TimestampTimezone,
	useSettingsState,
} from "@/lib/settings";
import { isAppUpdatesEnabled } from "@/lib/release-channel";

const CHECKED_AT_FORMATTER = new Intl.DateTimeFormat(undefined, {
	dateStyle: "medium",
	timeStyle: "short",
});

function SettingsRow({
	title,
	description,
	children,
}: {
	title: string;
	description: string;
	children: ReactNode;
}) {
	return (
		<div className="flex min-h-16 items-center justify-between gap-6 border-b py-4">
			<div className="min-w-0">
				<div className="text-sm font-medium text-foreground">{title}</div>
				<div className="mt-1 max-w-2xl text-xs leading-relaxed text-muted-foreground">
					{description}
				</div>
			</div>
			<div className="shrink-0">{children}</div>
		</div>
	);
}

function TimezoneOption({
	value,
	label,
	selected,
	onSelect,
}: {
	value: TimestampTimezone;
	label: string;
	selected: boolean;
	onSelect: (value: TimestampTimezone) => void;
}) {
	return (
		<Button
			type="button"
			variant={selected ? "secondary" : "ghost"}
			size="sm"
			className="h-8 rounded-sm px-2.5"
			onClick={() => onSelect(value)}
			aria-pressed={selected}
		>
			{selected && <Check data-icon="inline-start" />}
			{label}
		</Button>
	);
}

function formatCheckedAt(value: string | null): string {
	if (!value) return "Not checked yet";
	const date = new Date(value);
	if (Number.isNaN(date.getTime())) return "Not checked yet";
	return CHECKED_AT_FORMATTER.format(date);
}

export function SettingsPage() {
	const {
		showExactTimestamps,
		showUsageFooter,
		autoStartSavedPortForwards,
		timestampTimezone,
		kubeconfigEnvVar,
		setShowExactTimestamps,
		setShowUsageFooter,
		setAutoStartSavedPortForwards,
		setTimestampTimezone,
		setKubeconfigEnvVar,
		resetKubeconfigEnvVar,
	} = useSettingsState();
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
		<div className="mx-auto flex w-full max-w-4xl flex-col gap-6">
			<header className="border-b pb-4">
				<h1 className="text-lg font-semibold text-foreground">Settings</h1>
			</header>

			<section className="flex flex-col">
				<SettingsRow
					title="Show exact timestamps"
					description="Adds the exact timestamp next to relative ages. Exact timestamps remain available in tooltips when this is off."
				>
					<ToggleButton
						checked={showExactTimestamps}
						onCheckedChange={setShowExactTimestamps}
						ariaLabel="Show exact timestamps"
					/>
				</SettingsRow>
				<SettingsRow
					title="Show CPU and memory footer"
					description="Adds a compact footer with KubeCove CPU and memory usage for the app process tree."
				>
					<ToggleButton
						checked={showUsageFooter}
						onCheckedChange={setShowUsageFooter}
						ariaLabel="Show CPU and memory footer"
					/>
				</SettingsRow>
				<SettingsRow
					title="Auto-start saved port forwards"
					description="Starts saved Service port-forward presets automatically when a workspace is restored."
				>
					<ToggleButton
						checked={autoStartSavedPortForwards}
						onCheckedChange={setAutoStartSavedPortForwards}
						ariaLabel="Auto-start saved port forwards"
					/>
				</SettingsRow>
				<SettingsRow
					title="Kubeconfig env var"
					description={`Reads kubeconfig paths from ${normalizeKubeconfigEnvVar(kubeconfigEnvVar)}. Empty or unset values fall back to normal kubeconfig discovery.`}
				>
					<div className="flex items-center gap-2">
						<Input
							className="h-8 w-48"
							value={kubeconfigEnvVar}
							placeholder={DEFAULT_KUBECONFIG_ENV_VAR}
							onChange={(event) => setKubeconfigEnvVar(event.target.value)}
							aria-label="Kubeconfig env var"
						/>
						<Button
							type="button"
							variant="ghost"
							size="icon-sm"
							aria-label="Reset kubeconfig env var"
							onClick={resetKubeconfigEnvVar}
						>
							<RotateCcw />
						</Button>
					</div>
				</SettingsRow>
				<SettingsRow
					title="Timestamp timezone"
					description="Controls exact timestamps in inline labels and tooltips."
				>
					<div className="flex rounded-md border bg-background/50 p-0.5">
						<TimezoneOption
							value="local"
							label="Local"
							selected={timestampTimezone === "local"}
							onSelect={setTimestampTimezone}
						/>
						<TimezoneOption
							value="utc"
							label="UTC"
							selected={timestampTimezone === "utc"}
							onSelect={setTimestampTimezone}
						/>
					</div>
				</SettingsRow>
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
					<SettingsRow
						title="Update check failed"
						description={errorMessage}
					>
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
			</section>
		</div>
	);
}
