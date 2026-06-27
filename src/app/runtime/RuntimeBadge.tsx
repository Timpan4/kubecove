import reactLogoUrl from "./react-logo.svg";
import svelteLogoUrl from "./svelte-logo.svg";
import { Button } from "@/components/ui/button";
import { useSettingsState } from "@/lib/settings";
import {
	queueUiRuntimeSettingsFocus,
	uiRuntimeModeLabel,
} from "@/lib/ui-runtime";

export function RuntimeBadge({ onOpenSettings }: { onOpenSettings: () => void }) {
	const mode = useSettingsState((state) => state.uiRuntimeMode);
	const label = uiRuntimeModeLabel(mode);
	const logoUrl = mode === "svelte" ? svelteLogoUrl : reactLogoUrl;

	return (
		<Button
			type="button"
			variant="outline"
			size="sm"
			className="mr-2 h-8 gap-1.5 px-2 text-xs text-muted-foreground [-webkit-app-region:no-drag]"
			aria-label={`Open Settings for ${label} UI mode`}
			data-ui-runtime={mode}
			onClick={() => {
				queueUiRuntimeSettingsFocus();
				onOpenSettings();
			}}
		>
			<img src={logoUrl} alt="" className="size-3.5" aria-hidden="true" />
			<span>{label}</span>
		</Button>
	);
}
