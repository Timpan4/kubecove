import { readable, type Readable } from "svelte/store";
import {
	getForegroundLoadingSnapshot,
	subscribeForegroundLoading,
} from "./foreground-loading";

// ponytail: Svelte-readable wrapper over the runtime-agnostic foreground-loading store.
// Svelte-readable wrapper over the runtime-agnostic foreground-loading store.
export const foregroundLoading: Readable<number> = readable(
	getForegroundLoadingSnapshot(),
	(set) => subscribeForegroundLoading(() => set(getForegroundLoadingSnapshot())),
);
