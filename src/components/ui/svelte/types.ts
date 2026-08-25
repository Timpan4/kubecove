import type { Snippet } from "svelte";

export interface UiPropValue {
	toString(): string;
}

export interface UiProps {
	class?: string;
	children?: Snippet;
	[key: string]: UiPropValue | null | undefined;
}

export interface UiFieldError {
	message?: string;
}
