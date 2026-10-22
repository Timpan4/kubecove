import {
	closeStreamChannel,
	createStreamChannel,
	startResourceWatch,
	stopStream,
} from "./tauri-streams";
import type { TauriClient } from "./tauri-runtime";
import type { StreamMessage, WatchResourceKey } from "./types";
import { messageFromError } from "./error-redaction";

// Match the existing Argo inspection polling cadence while watches recover.
export const RESOURCE_RECOVERY_INTERVAL = 15_000;

export interface ResourceWatchState {
	status: "connecting" | "connected" | "partial" | "polling" | "reconnecting";
	message: string;
	error: string | null;
	reloadError?: string | null;
	warnings?: string[];
}

export function watchKeySignature(key: WatchResourceKey): string {
	const kind = key.resourceKind;
	return [kind.kind, kind.apiVersion ?? "", kind.plural ?? "", key.namespace ?? ""].join(":");
}

export interface ResourceScopeHandlers {
	onState: (state: ResourceWatchState) => void;
	onChange: (event: StreamMessage, key: WatchResourceKey) => void;
	reload: (keys?: WatchResourceKey[]) => Promise<void> | Promise<string[]>;
}

export interface ResourceScopeObserver {
	/** Reconciles the watched keys: unchanged keys keep their streams. */
	update: (keys: WatchResourceKey[], handlers: ResourceScopeHandlers) => void;
	stop: () => void;
}

interface WatchRegistration {
	key: WatchResourceKey;
	closed: boolean;
	id: string | null;
	opening: boolean;
	connected: boolean;
	error: string | null;
	permissionWarning: string | null;
	channel: ReturnType<typeof createStreamChannel> | null;
}

export function observeResourceScope({
	client,
	clusterContext,
	keys,
	kubeconfigEnvVar,
	onState,
	onChange,
	reload,
	openChannel = createStreamChannel,
}: ResourceScopeHandlers & {
	client: TauriClient;
	clusterContext: string;
	keys: WatchResourceKey[];
	kubeconfigEnvVar?: string;
	openChannel?: typeof createStreamChannel;
}): ResourceScopeObserver {
	let handlers: ResourceScopeHandlers = { onState, onChange, reload };
	let disposed = false;
	let timer: ReturnType<typeof setTimeout> | undefined;
	let reloading = false;
	let needsCatchUp = false;
	let reloadError: string | null = null;
	let reloadWarnings: string[] = [];
	// Bumped when update changes the key set, so reloads started for the old scope are discarded.
	let scopeGeneration = 0;
	const register = (key: WatchResourceKey): WatchRegistration => ({
		key,
		closed: false,
		id: null,
		opening: false,
		connected: false,
		error: null,
		permissionWarning: null,
		channel: null,
	});
	let watches = keys.map(register);

	async function refresh(recoveryKeys?: WatchResourceKey[]) {
		if (disposed || reloading) return;
		reloading = true;
		const scope = scopeGeneration;
		try {
			const warnings = (await handlers.reload(recoveryKeys)) ?? [];
			// A reload for a scope that keys have since left would revive its warnings.
			if (scope === scopeGeneration) {
				reloadWarnings = warnings;
				reloadError = null;
			}
		} catch (error) {
			if (scope === scopeGeneration) reloadError = messageFromError(error);
		} finally {
			reloading = false;
			publish();
		}
	}
	function publish() {
		if (disposed) return;
		const available = watches.filter((watch) => !watch.permissionWarning);
		const connected = available.every((watch) => watch.connected);
		const warnings = [...new Set([
			...watches.flatMap((watch) => watch.permissionWarning ? [watch.permissionWarning] : []),
			...reloadWarnings,
		])];
		const status = !connected ? "reconnecting"
			: available.length === 0 ? "polling"
			: warnings.length ? "partial" : "connected";
		handlers.onState({
			status,
			message: status === "polling" ? "Watch access is unavailable. Readable resources refresh periodically."
				: status === "partial" ? "Live updates connected for permitted resources. Readable resources without watch access refresh periodically."
				: connected ? "Live updates connected" : "Reconnecting; periodically reloading this view",
			error: watches.find((watch) => watch.error)?.error ?? null,
			reloadError,
			warnings,
		});
		if (connected && needsCatchUp && !reloading) {
			needsCatchUp = false;
			void refresh();
		}
	}
	function start(watch: WatchRegistration) {
		if (disposed || watch.closed || watch.opening || watch.id || watch.permissionWarning) return;
		watch.opening = true;
		const channel = openChannel((event) => {
			if (disposed || watch.closed || watch.channel !== channel) return;
			if (event.type === "status") {
				watch.permissionWarning = event.status === "forbidden" ? event.message : null;
				watch.connected = event.status === "connected";
				if (!watch.connected && !watch.permissionWarning) needsCatchUp = true;
				if (watch.connected || watch.permissionWarning) watch.error = null;
			} else if (event.type === "error") {
				watch.connected = false;
				watch.error = event.message;
				needsCatchUp = true;
			} else if (event.type === "stopped") {
				watch.connected = false;
				watch.id = null;
				needsCatchUp = true;
				closeStreamChannel(channel);
			} else if (event.type === "resourceChanged") {
				handlers.onChange(event, watch.key);
			}
			publish();
		}, client);
		watch.channel = channel;
		void startResourceWatch(
			client,
			clusterContext,
			[watch.key],
			channel,
			kubeconfigEnvVar,
		)
			.then((id) => {
				watch.opening = false;
				if (disposed || watch.closed) {
					void stopStream(client, id).catch(() => {});
					return;
				}
				watch.id = id;
			})
			.catch((error) => {
				watch.opening = false;
				if (watch.closed) return;
				watch.error = messageFromError(error);
				watch.connected = false;
				needsCatchUp = true;
				closeStreamChannel(channel);
				publish();
			});
	}
	function tick() {
		if (disposed) return;
		for (const watch of watches) start(watch);
		if (watches.some((watch) => !watch.connected && !watch.permissionWarning)) needsCatchUp = true;
		if (needsCatchUp || reloadError || watches.some((watch) => watch.permissionWarning)) {
			const recoveryKeys = watches.filter((watch) => watch.permissionWarning).map((watch) => watch.key);
			void refresh(!needsCatchUp && recoveryKeys.length ? recoveryKeys : undefined);
		}
		timer = setTimeout(tick, RESOURCE_RECOVERY_INTERVAL);
	}
	onState({
		status: "connecting",
		message: "Starting live updates",
		error: null,
		reloadError: null,
	});
	for (const watch of watches) start(watch);
	timer = setTimeout(tick, RESOURCE_RECOVERY_INTERVAL);

	function close(watch: WatchRegistration) {
		watch.closed = true;
		if (watch.channel) closeStreamChannel(watch.channel);
		if (watch.id) void stopStream(client, watch.id).catch(() => {});
	}

	return {
		update(nextKeys, nextHandlers) {
			if (disposed) return;
			handlers = nextHandlers;
			const wanted = new Map(nextKeys.map((key) => [watchKeySignature(key), key]));
			const kept: WatchRegistration[] = [];
			for (const watch of watches) {
				const signature = watchKeySignature(watch.key);
				if (wanted.delete(signature)) kept.push(watch);
				else close(watch);
			}
			const added = [...wanted.values()].map(register);
			if (added.length > 0 || kept.length < watches.length) {
				// Reload results describe the old scope; the next recovery tick re-derives them.
				scopeGeneration += 1;
				reloadWarnings = [];
				reloadError = null;
			}
			watches = [...kept, ...added];
			for (const watch of added) start(watch);
			if (added.length === 0) {
				publish();
				return;
			}
			handlers.onState({
				status: "connecting",
				message: "Starting live updates",
				error: null,
				reloadError,
				warnings: [...new Set(kept.flatMap((watch) => watch.permissionWarning ? [watch.permissionWarning] : []))],
			});
		},
		stop() {
			disposed = true;
			if (timer) clearTimeout(timer);
			for (const watch of watches) close(watch);
		},
	};
}
