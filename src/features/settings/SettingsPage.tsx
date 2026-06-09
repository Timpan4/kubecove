import { ArrowLeft } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ToggleButton } from "@/components/ToggleButton";
import { useSettingsState } from "@/lib/settings";
import { AppUpdatesSection } from "./AppUpdatesSection";
import { KubeconfigSourcesSection } from "./KubeconfigSourcesSection";
import { SegmentedControl, SettingsRow, SettingsSection } from "./SettingsControls";

export function SettingsPage({ onBack }: { onBack: () => void }) {
	const {
		showExactTimestamps,
		showUsageFooter,
		showOwnershipMapByDefault,
		autoStartSavedPortForwards,
		keepLiveSessionsOnWorkspaceSwitch,
		allowYamlForceConflicts,
		timestampTimezone,
		yamlViewModeDefault,
		yamlEncodingDefault,
		setShowExactTimestamps,
		setShowUsageFooter,
		setShowOwnershipMapByDefault,
		setAutoStartSavedPortForwards,
		setKeepLiveSessionsOnWorkspaceSwitch,
		setAllowYamlForceConflicts,
		setTimestampTimezone,
		setYamlViewModeDefault,
		setYamlEncodingDefault,
	} = useSettingsState();

	return (
		<div className="mx-auto flex w-full max-w-3xl flex-col gap-8 pb-8">
			<header className="flex items-center gap-3 pt-2">
				<Button
					type="button"
					variant="ghost"
					size="icon"
					className="size-8"
					aria-label="Back"
					title="Back"
					onClick={onBack}
				>
					<ArrowLeft />
				</Button>
				<h1 className="text-xl font-semibold text-foreground">Settings</h1>
			</header>

			<SettingsSection title="General">
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
					title="Timestamp timezone"
					description="Controls exact timestamps in inline labels and tooltips."
				>
					<SegmentedControl
						value={timestampTimezone}
						options={[
							{ value: "local", label: "Local" },
							{ value: "utc", label: "UTC" },
						]}
						onChange={setTimestampTimezone}
						ariaLabel="Timestamp timezone"
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
					title="Show ownership map by default"
					description="Opens the ownership map when entering resource views. When off, the map stays collapsed until opened."
				>
					<ToggleButton
						checked={showOwnershipMapByDefault}
						onCheckedChange={setShowOwnershipMapByDefault}
						ariaLabel="Show ownership map by default"
					/>
				</SettingsRow>
			</SettingsSection>

			<SettingsSection title="Live sessions">
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
					title="Keep live sessions across workspace switches"
					description="Leaves port-forward and Pod exec sessions running when you leave their workspace scope. Fixed local port conflicts are skipped and shown in Port Forwards."
				>
					<ToggleButton
						checked={keepLiveSessionsOnWorkspaceSwitch}
						onCheckedChange={setKeepLiveSessionsOnWorkspaceSwitch}
						ariaLabel="Keep live sessions across workspace switches"
					/>
				</SettingsRow>
			</SettingsSection>

			<SettingsSection title="YAML">
				<SettingsRow
					title="YAML cleanup shape"
					description="Controls whether YAML panels open with kubectl-style inspect output or apply-friendly output."
				>
					<SegmentedControl
						value={yamlViewModeDefault}
						options={[
							{ value: "kubectl", label: "Kubectl" },
							{ value: "applyClean", label: "Apply-friendly" },
						]}
						onChange={setYamlViewModeDefault}
						ariaLabel="YAML cleanup shape"
					/>
				</SettingsRow>
				<SettingsRow
					title="YAML encoding"
					description="Controls whether YAML panels open as regular YAML or Kubernetes KYAML flow-style text."
				>
					<SegmentedControl
						value={yamlEncodingDefault}
						options={[
							{ value: "yaml", label: "YAML" },
							{ value: "kyaml", label: "KYAML" },
						]}
						onChange={setYamlEncodingDefault}
						ariaLabel="YAML encoding"
					/>
				</SettingsRow>
				<SettingsRow
					title="Allow YAML force-conflicts"
					description="Lets selected-resource YAML dry-run and apply take server-side field ownership when another manager owns changed fields."
				>
					<ToggleButton
						checked={allowYamlForceConflicts}
						onCheckedChange={setAllowYamlForceConflicts}
						ariaLabel="Allow YAML force-conflicts"
					/>
				</SettingsRow>
			</SettingsSection>

			<KubeconfigSourcesSection />

			<AppUpdatesSection />
		</div>
	);
}
