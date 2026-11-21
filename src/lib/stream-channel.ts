export interface StreamChannel<T> {
	readonly id: number;
	onmessage(message: T): void;
	close(): void;
}

export function createChannel<T>(
	id: number,
	onMessage: (message: T) => void,
	onClose?: () => void,
): StreamChannel<T> {
	let closed = false;
	return {
		id,
		onmessage(message) {
			if (!closed) onMessage(message);
		},
		close() {
			if (closed) return;
			closed = true;
			onClose?.();
		},
	};
}
