import type { Snippet } from "svelte";

export interface UiProps {
	class?: string;
	children?: Snippet;
	[key: string]: unknown;
}

export interface UiFieldError {
	message?: string;
}
