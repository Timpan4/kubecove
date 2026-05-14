import { create } from "zustand";
import { persist } from "zustand/middleware";

export type TimestampTimezone = "local" | "utc";

interface SettingsState {
	showExactTimestamps: boolean;
	timestampTimezone: TimestampTimezone;
	setShowExactTimestamps: (show: boolean) => void;
	setTimestampTimezone: (timezone: TimestampTimezone) => void;
}

export const useSettingsState = create<SettingsState>()(
	persist(
		(set) => ({
			showExactTimestamps: false,
			timestampTimezone: "local",
			setShowExactTimestamps: (show: boolean) =>
				set({ showExactTimestamps: show }),
			setTimestampTimezone: (timezone: TimestampTimezone) =>
				set({ timestampTimezone: timezone }),
		}),
		{ name: "k8s-manager-settings" },
	),
);
