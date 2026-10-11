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

interface WatchRegistration {
	key: WatchResourceKey;
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
}: {
	client: TauriClient;
	clusterContext: string;
	keys: WatchResourceKey[];
	kubeconfigEnvVar?: string;
	onState: (state: ResourceWatchState) => void;
	onChange: (event: StreamMessage) => void;
	reload: () => Promise<void> | Promise<string[]>;
	openChannel?: typeof createStreamChannel;
}): () => void {
	let disposed = false;
	let timer: ReturnType<typeof setTimeout> | undefined;
	let reloading = false;
	let needsCatchUp = false;
	let reloadError: string | null = null;
	let reloadWarnings: string[] = [];
	const watches: WatchRegistration[] = keys.map((key) => ({
		key,
		id: null,
		opening: false,
		connected: false,
		error: null,
		permissionWarning: null,
		channel: null,
	}));

	async function refresh() {
		if (disposed || reloading) return;
		reloading = true;
		try {
			reloadWarnings = (await reload()) ?? [];
			reloadError = null;
		} catch (error) {
			reloadError = messageFromError(error);
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
		onState({
			status,
			message: status === "polling" ? "Watch access is unavailable. Readable resources refresh periodically."
				: status === "partial" ? "Live updates connected for permitted resources. Readable resources without watch access refresh periodically."
				: connected ? "Live updates connected" : "Reconnecting; periodically reloading this view",
			error: watches.find((watch) => watch.error)?.error ?? null,
			reloadError,
			warnings,
		});
		if (connected && needsCatchUp) {
			needsCatchUp = false;
			void refresh();
		}
	}
	function start(watch: (typeof watches)[number]) {
		if (disposed || watch.opening || watch.id || watch.permissionWarning) return;
		watch.opening = true;
		const channel = openChannel((event) => {
			if (disposed || watch.channel !== channel) return;
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
				onChange(event);
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
				if (disposed) {
					void stopStream(client, id).catch(() => {});
					return;
				}
				watch.id = id;
			})
			.catch((error) => {
				watch.opening = false;
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
		if (needsCatchUp || reloadError || watches.some((watch) => watch.permissionWarning)) void refresh();
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
	return () => {
		disposed = true;
		if (timer) clearTimeout(timer);
		for (const watch of watches) {
			if (watch.channel) closeStreamChannel(watch.channel);
			if (watch.id) void stopStream(client, watch.id).catch(() => {});
		}
	};
}
