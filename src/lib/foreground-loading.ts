import { useSyncExternalStore } from "react";

type Listener = () => void;

const listeners = new Set<Listener>();
const activeLoads = new Map<number, string>();
let sequence = 0;

function emit() {
	for (const listener of listeners) {
		listener();
	}
}

function subscribe(listener: Listener) {
	listeners.add(listener);
	return () => listeners.delete(listener);
}

function snapshot() {
	return activeLoads.size;
}

export function getForegroundLoadingSnapshot(): number {
	return snapshot();
}

export function beginForegroundLoad(label: string): () => void {
	sequence = (sequence + 1) % Number.MAX_SAFE_INTEGER;
	const id = sequence;
	activeLoads.set(id, label);
	emit();
	let ended = false;
	return () => {
		if (ended) return;
		ended = true;
		activeLoads.delete(id);
		emit();
	};
}

export async function withForegroundLoad<T>(
	label: string,
	task: () => Promise<T>,
): Promise<T> {
	const end = beginForegroundLoad(label);
	try {
		return await task();
	} finally {
		end();
	}
}

export function useForegroundLoading(): number {
	return useSyncExternalStore(subscribe, snapshot, snapshot);
}
