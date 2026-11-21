import { diagnosticLog, diagnosticResultSummary, isDiagnosticsEnabled } from "./diagnostics";
import { recordInteractionCommand } from "./interaction-recorder";
import { createChannel, type StreamChannel } from "./stream-channel";

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

// The diagnostics page's own commands. It polls and clears with them, so tracing
// them would refill the latency report right after Clear. The GPUI client does
// not measure them either.
const UNTRACED_COMMANDS = new Set([
	"get_backend_diagnostics",
	"get_backend_cache_diagnostics",
	"clear_backend_diagnostics",
	"set_backend_diagnostics_enabled",
]);

/**
 * Runs one command and, while diagnostics is enabled, logs its round trip as
 * tauri.invoke.* events for the latency report and the interaction recorder.
 */
export async function traceInvoke<T, Args extends object>(
	cmd: string,
	args: Args | undefined,
	call: () => Promise<T>,
): Promise<T> {
	if (!isDiagnosticsEnabled() || UNTRACED_COMMANDS.has(cmd)) return call();
	const started = performance.now();
	diagnosticLog("tauri.invoke.start", {
		cmd,
		args: args ? Object.keys(args).join(",") : "",
	});
	try {
		const result = await call();
		const ms = performance.now() - started;
		diagnosticLog("tauri.invoke.done", {
			cmd,
			ms: Math.round(ms),
			result: diagnosticResultSummary(result),
		});
		if (isDiagnosticsEnabled()) recordInteractionCommand(cmd, started, ms, true);
		return result;
	} catch (error) {
		const ms = performance.now() - started;
		diagnosticLog("tauri.invoke.error", {
			cmd,
			ms: Math.round(ms),
			error: errorMessage(error),
		});
		if (isDiagnosticsEnabled()) recordInteractionCommand(cmd, started, ms, false);
		throw error;
	}
}

export interface TauriClient {
	createChannel?<T>(onMessage: (message: T) => void): StreamChannel<T>;
	invoke<T, Args extends object = object>(cmd: string, args?: Args): Promise<T>;
}

type MockInvokeResult = boolean | number | object | string | null | undefined;
type MockInvokeHandler = <Args extends object = object>(
	args?: Args,
) => MockInvokeResult | Promise<MockInvokeResult>;
type MockInvokeResponse = MockInvokeHandler | MockInvokeResult;

let nextMockChannelId = 1;

export function createMockChannel<T>(onMessage: (message: T) => void): StreamChannel<T> {
	return createChannel(nextMockChannelId++, onMessage);
}

export function createMockTauriClient<Responses extends object>(
	mockResponses: Responses,
): TauriClient {
	return {
		createChannel: createMockChannel,
		invoke: async <T, Args extends object = object>(
			cmd: string,
			args?: Args,
		): Promise<T> => {
			if (Object.hasOwn(mockResponses, cmd)) {
				const response: MockInvokeResponse = Object.entries(mockResponses).find(
					([command]) => command === cmd,
				)?.[1];
				if (response instanceof Function) {
					// SAFETY: caller's typed wrapper owns result T; mock handler mirrors that command contract.
					return (await response(args)) as T;
				}
				// SAFETY: caller's typed wrapper owns result T; fixture mirrors that command contract.
				return response as T;
			}
			throw new Error(`No mock response for command: ${cmd}`);
		},
	};
}
