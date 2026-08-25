export interface WorkspaceTransitionHooks<T> {
	suspend: () => Promise<void> | void;
	cancel: () => Promise<void>;
	apply: (destination: T) => void;
	resume: () => void;
	onCancelError?: (error: Error) => void;
}

export interface WorkspaceTransitionCoordinator<T> {
	request: (destination: T) => Promise<void>;
	isPending: () => boolean;
}

export async function cancelWorkspaceWork(
	cancelQueries: () => Promise<void>,
	cancelBackend: () => Promise<void>,
): Promise<void> {
	try {
		void cancelQueries().catch(() => undefined);
	} catch {
		// Query cancellation is best-effort; backend cancellation owns the abort boundary.
	}
	await cancelBackend();
}

export function createWorkspaceTransitionCoordinator<T>(
	hooks: WorkspaceTransitionHooks<T>,
): WorkspaceTransitionCoordinator<T> {
	let latest: T | undefined;
	let running: Promise<void> | null = null;

	async function run() {
		while (latest !== undefined) {
			try {
				try {
					await hooks.suspend();
				} catch (error) {
					hooks.onCancelError?.(
						error instanceof Error ? error : new Error(String(error)),
					);
				}
				try {
					await hooks.cancel();
				} catch (error) {
					hooks.onCancelError?.(
						error instanceof Error ? error : new Error(String(error)),
					);
				}
				const destination = latest;
				latest = undefined;
				if (destination !== undefined) {
					try {
						hooks.apply(destination);
					} catch (error) {
						hooks.onCancelError?.(
							error instanceof Error ? error : new Error(String(error)),
						);
					}
				}
			} finally {
				hooks.resume();
			}
		}
	}

	function start() {
		const drain = run();
		let completion!: Promise<void>;
		completion = drain.finally(() => {
			if (running !== completion) return;
			running = null;
			if (latest !== undefined) start();
		});
		running = completion;
	}

	async function waitForIdle() {
		while (running) await running;
	}

	return {
		request(destination) {
			latest = destination;
			if (!running) start();
			return waitForIdle();
		},
		isPending: () => running !== null,
	};
}
