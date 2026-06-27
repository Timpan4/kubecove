import { readable, type Readable } from "svelte/store";
import {
	getForegroundLoadingSnapshot,
	subscribeForegroundLoading,
} from "./foreground-loading";

// ponytail: Svelte-readable wrapper over the runtime-agnostic foreground-loading store.
// Mirrors useForegroundLoading (useSyncExternalStore) for Svelte consumers.
export const foregroundLoading: Readable<number> = readable(
	getForegroundLoadingSnapshot(),
	(set) => subscribeForegroundLoading(() => set(getForegroundLoadingSnapshot())),
);
