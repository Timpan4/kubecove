import { writable } from "svelte/store";

export type ToastTone = "default" | "success" | "warning" | "error";

export interface ToastMessage {
	id: number;
	title: string;
	description?: string;
	tone: ToastTone;
}

const messages = writable<ToastMessage[]>([]);
let nextId = 1;

export const toasts = { subscribe: messages.subscribe };

export function dismissToast(id: number): void {
	messages.update((current) => current.filter((message) => message.id !== id));
}

export function showToast(
	message: Omit<ToastMessage, "id">,
	durationMs = 4_500,
): number {
	const id = nextId++;
	messages.update((current) => [...current.slice(-3), { ...message, id }]);
	if (typeof window !== "undefined" && durationMs > 0) {
		window.setTimeout(() => dismissToast(id), durationMs);
	}
	return id;
}
