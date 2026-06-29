import { Channel, type InvokeOptions } from "@tauri-apps/api/core";

interface BrowserDevEnv {
	DEV?: boolean;
}

interface TauriRuntimeGlobal {
	__TAURI_INTERNALS__?: unknown;
	isTauri?: boolean;
}

export interface TauriClient {
	invoke<T>(
		cmd: string,
		args?: Record<string, unknown>,
		options?: InvokeOptions,
	): Promise<T>;
}

type MockInvokeHandler = (
	args?: Record<string, unknown>,
	options?: InvokeOptions,
) => unknown | Promise<unknown>;

type MockInvokeResponse = MockInvokeHandler | unknown;

interface MockChannel<T> {
	id: number;
	onmessage: (message: T) => void;
	cleanupCallback: () => void;
	unregister: () => Promise<void>;
	toJSON: () => string;
}

let nextMockChannelId = 1;

function importMetaEnv(): BrowserDevEnv {
	return ((import.meta as { env?: BrowserDevEnv }).env ?? {}) as BrowserDevEnv;
}

export function isTauriRuntime(
	scope: TauriRuntimeGlobal = globalThis as TauriRuntimeGlobal,
): boolean {
	return scope.__TAURI_INTERNALS__ !== undefined || scope.isTauri === true;
}

export function shouldUseBrowserDevMocks(
	env: BrowserDevEnv = importMetaEnv(),
	scope: TauriRuntimeGlobal = globalThis as TauriRuntimeGlobal,
): boolean {
	return env.DEV === true && !isTauriRuntime(scope);
}

export function isBrowserDevMockMode(): boolean {
	return shouldUseBrowserDevMocks();
}

export function createMockChannel<T>(
	onMessage: (message: T) => void,
): Channel<T> {
	let active = true;
	const channel: MockChannel<T> = {
		id: nextMockChannelId++,
		onmessage: (message) => {
			if (active) onMessage(message);
		},
		cleanupCallback: () => {
			active = false;
		},
		unregister: async () => {
			active = false;
		},
		toJSON: () => `__MOCK_CHANNEL__:${channel.id}`,
	};
	return channel as unknown as Channel<T>;
}

export function createMockTauriClient(
	mockResponses: Record<string, MockInvokeResponse>,
): TauriClient {
	return {
		invoke: async <T>(
			cmd: string,
			args?: Record<string, unknown>,
			options?: InvokeOptions,
		): Promise<T> => {
			if (cmd in mockResponses) {
				const response = mockResponses[cmd];
				if (typeof response === "function") {
					return (await (response as MockInvokeHandler)(args, options)) as T;
				}
				return response as T;
			}
			throw new Error(`No mock response for command: ${cmd}`);
		},
	};
}
