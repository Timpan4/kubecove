import { create } from "zustand";
import { persist } from "zustand/middleware";

export type TimestampTimezone = "local" | "utc";

interface SettingsState {
	showExactTimestamps: boolean;
	showUsageFooter: boolean;
	autoStartSavedPortForwards: boolean;
	timestampTimezone: TimestampTimezone;
	setShowExactTimestamps: (show: boolean) => void;
	setShowUsageFooter: (show: boolean) => void;
	setAutoStartSavedPortForwards: (autoStart: boolean) => void;
	setTimestampTimezone: (timezone: TimestampTimezone) => void;
}

export const useSettingsState = create<SettingsState>()(
	persist(
		(set) => ({
			showExactTimestamps: false,
			showUsageFooter: false,
			autoStartSavedPortForwards: false,
			timestampTimezone: "local",
			setShowExactTimestamps: (show: boolean) =>
				set({ showExactTimestamps: show }),
			setShowUsageFooter: (show: boolean) => set({ showUsageFooter: show }),
			setAutoStartSavedPortForwards: (autoStart: boolean) =>
				set({ autoStartSavedPortForwards: autoStart }),
			setTimestampTimezone: (timezone: TimestampTimezone) =>
				set({ timestampTimezone: timezone }),
		}),
		{ name: "kubecove-settings" },
	),
);
