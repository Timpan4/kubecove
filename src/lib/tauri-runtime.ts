import type { Channel, InvokeOptions } from "@tauri-apps/api/core";
import { IS_DEV_BUILD } from "./build-env";

interface BrowserDevEnv {
	DEV?: boolean;
}

interface TauriRuntimeGlobal {
	__TAURI_INTERNALS__?: unknown;
	isTauri?: boolean;
}

export interface TauriClient {
	invoke<T, Args extends object = object>(
		cmd: string,
		args?: Args,
		options?: InvokeOptions,
	): Promise<T>;
}

type MockInvokeResult = boolean | number | object | string | null | undefined;
type MockInvokeHandler = <Args extends object = object>(
	args?: Args,
	options?: InvokeOptions,
) => MockInvokeResult | Promise<MockInvokeResult>;

type MockInvokeResponse = MockInvokeHandler | MockInvokeResult;

interface MockChannel<T> {
	id: number;
	onmessage: (message: T) => void;
	cleanupCallback: () => void;
	unregister: () => Promise<void>;
	toJSON: () => string;
}

let nextMockChannelId = 1;

export function isTauriRuntime(
	// SAFETY: globalThis is the runtime object whose optional Tauri markers this function reads.
	scope: TauriRuntimeGlobal = globalThis as TauriRuntimeGlobal,
): boolean {
	return scope.__TAURI_INTERNALS__ !== undefined || scope.isTauri === true;
}

export function shouldUseBrowserDevMocks(
	env: BrowserDevEnv = { DEV: IS_DEV_BUILD },
	// SAFETY: globalThis is the runtime object whose optional Tauri markers this function reads.
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
	// SAFETY: browser mock implements every Channel member used by frontend stream wrappers.
	// @ts-expect-error: Tauri Channel has private runtime members absent from the browser mock.
	return channel as Channel<T>;
}

export function createMockTauriClient<Responses extends object>(
	mockResponses: Responses,
): TauriClient {
	return {
		invoke: async <T, Args extends object = object>(
			cmd: string,
			args?: Args,
			options?: InvokeOptions,
		): Promise<T> => {
			if (Object.hasOwn(mockResponses, cmd)) {
				const response: MockInvokeResponse = Object.entries(mockResponses).find(
					([command]) => command === cmd,
				)?.[1];
				if (response instanceof Function) {
					// SAFETY: caller's typed wrapper owns result T; mock handler mirrors that command contract.
					return (await response(args, options)) as T;
				}
				// SAFETY: caller's typed wrapper owns result T; fixture mirrors that command contract.
				return response as T;
			}
			throw new Error(`No mock response for command: ${cmd}`);
		},
	};
}
