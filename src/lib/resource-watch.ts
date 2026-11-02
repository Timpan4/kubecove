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
	catchingUp: boolean;
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
	readError?: () => string | null;
}

export interface ResourceScopeObserver {
	/** Reconciles the watched keys: unchanged keys keep their streams. */
	update: (keys: WatchResourceKey[], handlers: ResourceScopeHandlers) => void;
	stop: () => void;
	checkReads: () => void;
}

type WatchChannel = ReturnType<typeof createStreamChannel>;

// The stream's lifecycle. Idle watches restart on the next recovery tick; closed is final.
type WatchPhase =
	| { kind: "idle" }
	| { kind: "opening"; channel: WatchChannel }
	| { kind: "running"; channel: WatchChannel; id: string }
	| { kind: "closed" };

interface WatchRegistration {
	key: WatchResourceKey;
	phase: WatchPhase;
	// What the running stream last reported.
	connected: boolean;
	error: string | null;
	permissionWarning: string | null;
}

export function observeResourceScope({
	client,
	clusterContext,
	keys,
	kubeconfigEnvVar,
	onState,
	onChange,
	reload,
	readError,
	openChannel = createStreamChannel,
}: ResourceScopeHandlers & {
	client: TauriClient;
	clusterContext: string;
	keys: WatchResourceKey[];
	kubeconfigEnvVar?: string;
	openChannel?: typeof createStreamChannel;
}): ResourceScopeObserver {
	let handlers: ResourceScopeHandlers = { onState, onChange, reload, readError };
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
		phase: { kind: "idle" },
		connected: false,
		error: null,
		permissionWarning: null,
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
			catchingUp: needsCatchUp || reloading,
			message: status === "polling" ? "Watch access is unavailable. Readable resources refresh periodically."
				: status === "partial" ? "Live updates connected for permitted resources. Readable resources without watch access refresh periodically."
				: connected ? "Live updates connected" : "Reconnecting; periodically reloading this view",
			error: watches.find((watch) => watch.error)?.error ?? null,
			reloadError: reloadError ?? handlers.readError?.() ?? null,
			warnings,
		});
		if (connected && needsCatchUp && !reloading) {
			needsCatchUp = false;
			void refresh();
		}
	}
	function start(watch: WatchRegistration) {
		if (disposed || watch.phase.kind !== "idle" || watch.permissionWarning) return;
		const channel = openChannel((event) => {
			if (disposed || watch.phase.kind === "closed" || watch.phase.kind === "idle" || watch.phase.channel !== channel) return;
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
				// Also reached while opening: the start result below then finds the watch idle.
				watch.phase = { kind: "idle" };
				watch.connected = false;
				needsCatchUp = true;
				closeStreamChannel(channel);
			} else if (event.type === "resourceChanged") {
				handlers.onChange(event, watch.key);
			}
			publish();
		}, client);
		const opening: WatchPhase = { kind: "opening", channel };
		watch.phase = opening;
		void startResourceWatch(
			client,
			clusterContext,
			[watch.key],
			channel,
			kubeconfigEnvVar,
		)
			.then((id) => {
				if (disposed || watch.phase !== opening) {
					void stopStream(client, id).catch(() => {});
					return;
				}
				watch.phase = { kind: "running", channel, id };
			})
			.catch((error) => {
				if (watch.phase !== opening) return;
				watch.phase = { kind: "idle" };
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
		const readError = handlers.readError?.();
		if (needsCatchUp || reloadError || readError || watches.some((watch) => watch.permissionWarning)) {
			const recoveryKeys = watches.filter((watch) => watch.permissionWarning).map((watch) => watch.key);
			void refresh(!needsCatchUp && !readError && recoveryKeys.length ? recoveryKeys : undefined);
		}
		timer = setTimeout(tick, RESOURCE_RECOVERY_INTERVAL);
	}
	onState({
		status: "connecting",
		catchingUp: false,
		message: "Starting live updates",
		error: null,
		reloadError: null,
	});
	for (const watch of watches) start(watch);
	timer = setTimeout(tick, RESOURCE_RECOVERY_INTERVAL);

	function close(watch: WatchRegistration) {
		const previous = watch.phase;
		watch.phase = { kind: "closed" };
		if (previous.kind === "opening" || previous.kind === "running") closeStreamChannel(previous.channel);
		if (previous.kind === "running") void stopStream(client, previous.id).catch(() => {});
	}

	return {
		checkReads: publish,
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
				catchingUp: needsCatchUp || reloading,
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
