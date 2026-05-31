import { useState } from "react";
import { Cable, Eye, Play, X } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { ToggleButton } from "@/components/ToggleButton";
import { useSettingsState } from "@/lib/settings";
import type { SavedWorkspace } from "@/lib/workspaces";
import { savedPortForwardStartFailureMessage } from "./restore";
import { useSavedPortForwardActions } from "./useSavedPortForwardActions";

interface SavedPortForwardRestorePromptProps {
	workspace: SavedWorkspace;
	onReview: () => void;
	onDismiss: () => void;
}

export function SavedPortForwardRestorePrompt({
	workspace,
	onReview,
	onDismiss,
}: SavedPortForwardRestorePromptProps) {
	const autoStartSavedPortForwards = useSettingsState(
		(state) => state.autoStartSavedPortForwards,
	);
	const setAutoStartSavedPortForwards = useSettingsState(
		(state) => state.setAutoStartSavedPortForwards,
	);
	const { startAll, startingAll } = useSavedPortForwardActions(workspace);
	const [startError, setStartError] = useState<string | null>(null);
	const count = workspace.portForwards?.length ?? 0;

	const handleStart = async () => {
		setStartError(null);
		const results = await startAll();
		const failureMessage = savedPortForwardStartFailureMessage(results);
		if (failureMessage) {
			setStartError(failureMessage);
			return;
		}
		onDismiss();
	};

	return (
		<Alert className="rounded-none border-x-0 border-t-0">
			<Cable className="size-3.5" />
			<AlertTitle>Saved port forwards available</AlertTitle>
			<AlertDescription className="flex flex-wrap items-center justify-between gap-3">
				<span>
					This workspace has {count} saved Service{" "}
					{count === 1 ? "forward" : "forwards"} ready to start.
				</span>
				{startError && (
					<span className="w-full text-xs font-medium text-destructive">
						{startError}
					</span>
				)}
				<span className="flex flex-wrap items-center gap-2">
					<span className="inline-flex items-center gap-2 pr-2 text-xs text-muted-foreground">
						<ToggleButton
							checked={autoStartSavedPortForwards}
							onCheckedChange={setAutoStartSavedPortForwards}
							ariaLabel="Auto-start saved port forwards"
						/>
						Auto-start
					</span>
					<Button
						type="button"
						size="sm"
						onClick={() => void handleStart()}
						disabled={startingAll}
					>
						{startingAll ? (
							<Spinner data-icon="inline-start" />
						) : (
							<Play data-icon="inline-start" />
						)}
						Start saved
					</Button>
					<Button type="button" size="sm" variant="outline" onClick={onReview}>
						<Eye data-icon="inline-start" />
						Review
					</Button>
					<Button type="button" size="sm" variant="ghost" onClick={onDismiss}>
						<X data-icon="inline-start" />
						Skip
					</Button>
				</span>
			</AlertDescription>
		</Alert>
	);
}
