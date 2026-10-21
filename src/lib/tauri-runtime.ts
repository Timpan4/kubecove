import { Channel, invoke, type InvokeArgs, type InvokeOptions } from "@tauri-apps/api/core";
import { diagnosticLog, diagnosticResultSummary, isDiagnosticsEnabled } from "./diagnostics";
import { markProfileIpc, markStartup } from "./startup-marks";
import { IS_DEV_BUILD, IS_WEB_RUNTIME } from "./build-env";

function errorMessage(cause: unknown): string {
	if (cause instanceof Error) return cause.message;
	if (String(cause) === cause) return cause;
	if (hasMessage(cause)) {
		const message = cause.message;
		if (String(message) === message) return message;
	}
	return String(cause);
}

function hasMessage<Value>(value: Value): value is Value & { message?: string } {
	return value !== null && Object(value) === value && "message" in Object(value);
}

export function createNativeTauriClient(): TauriClient {
	return {
		createChannel: (onMessage) => new Channel(onMessage),
		invoke: async <T, Args extends object = object>(
			cmd: string,
			args?: Args,
			options?: InvokeOptions,
		): Promise<T> => {
			const diagnosticsEnabled = isDiagnosticsEnabled();
			const started = diagnosticsEnabled || process.env.KUBECOVE_PUBLIC_PROFILE === "true"
				? performance.now() : 0;
			if (diagnosticsEnabled) {
				diagnosticLog("tauri.invoke.start", {
					cmd,
					args: args ? Object.keys(args).join(",") : "",
				});
			}
			try {
				// SAFETY: typed wrappers pass Tauri-serializable command objects; Tauri validates IPC encoding.
				const result = await invoke<T>(cmd, args as InvokeArgs | undefined, options);
				markProfileIpc(cmd, started, true);
				if (cmd === "get_kubeconfig_sources") markStartup("kubeconfig-ready");
				if (diagnosticsEnabled) {
					diagnosticLog("tauri.invoke.done", {
						cmd,
						ms: Math.round(performance.now() - started),
						result: diagnosticResultSummary(result),
					});
				}
				return result;
			} catch (error) {
				markProfileIpc(cmd, started, false);
				if (diagnosticsEnabled) {
					diagnosticLog("tauri.invoke.error", {
						cmd,
						ms: Math.round(performance.now() - started),
						error: errorMessage(error),
					});
				}
				console.error(`[k8s-manager:tauri] ${cmd} error`, error);
				throw error;
			}
		},
	};
}

interface BrowserDevEnv {
	DEV?: boolean;
}

interface TauriRuntimeGlobal {
	__TAURI_INTERNALS__?: unknown;
	isTauri?: boolean;
}

export interface TauriClient {
	createChannel?<T>(onMessage: (message: T) => void): Channel<T>;
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
	return !IS_WEB_RUNTIME && env.DEV === true && !isTauriRuntime(scope);
}

export function isBrowserRuntime(): boolean {
	return IS_WEB_RUNTIME;
}

export function getChannelId<T>(channel: Channel<T>): number {
	return channel.id;
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
		createChannel: (onMessage) => createMockChannel(onMessage),
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
