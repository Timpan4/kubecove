// Reader's view of ResourceWatch.tla: what the resource view shows for one watched kind.
type Vars = {
	phase: string;
	closed: boolean;
	opening: boolean;
	id: number;
	error: boolean;
	warning: boolean;
	stream: Record<string, string>;
	q: Record<string, Record<string, string>>;
	starts: number;
};

const current = (v: Vars) => v.phase !== "unused";
const isClosed = (v: Vars) => (current(v) ? v.phase === "closed" : v.closed);
const running = (v: Vars) => (current(v) ? v.phase === "running" : !v.closed && v.id !== 0);
const opening = (v: Vars) => (current(v) ? v.phase === "opening" : v.opening && v.id === 0);
const queued = (v: Vars, n: number) => Object.values(v.q[String(n)] ?? {});

export const title = "Resource watch for one kind";

export const state = (v: Vars) => {
	if (isClosed(v)) return "Closed";
	if (running(v)) {
		if (v.stream[String(v.id)] === "live") return "Live";
		if (queued(v, v.id).length) return "Live, final message on its way";
		if (v.warning) return "Watch access denied, view polls";
		if (v.error) return "Stream failed, error shown";
		return "Shown live on an ended stream";
	}
	if (opening(v)) return "Starting the watch";
	return v.error ? "Waiting to restart, error shown" : "Waiting to restart";
};

const MESSAGE: Record<string, string> = {
	stopped: "stopped",
	forbidden: "watch access denied",
	error: "an error",
	connected: "a status",
	reconnecting: "a status",
};

export const step = (action: string, from: Vars, to: Vars) => {
	if (action === "RecoveryTick") return "the recovery timer restarts the watch";
	if (action === "RecoveryTickSkips") return null;
	if (action === "Close") return "the view stops watching";
	if (action === "Resolve") return "the start call returns";
	if (action === "Reject") return "the start call fails";
	if (action === "Accept") return "the backend opens the stream";
	if (action === "Refuse") return "the backend refuses the start";
	if (action === "Report") return "the stream sends a status";
	if (action === "Deliver") {
		for (const n of Object.keys(from.q)) {
			const before = Object.values(from.q[n]);
			if (before.length && before.length !== Object.values(to.q[n]).length)
				return `the app receives ${MESSAGE[before[0]] ?? before[0]}`;
		}
		return "the app receives a message";
	}
	if (action === "EndStream") {
		for (const n of Object.keys(to.stream)) {
			if (from.stream[n] === "live" && to.stream[n] === "ended") {
				const sent = Object.values(to.q[n]);
				const last = sent.length > Object.values(from.q[n]).length ? sent[sent.length - 1] : undefined;
				if (last === "stopped") return "the stream sends stopped and ends";
				if (last === "forbidden") return "the stream ends: watch access denied";
				if (last === "error") return "the stream ends with an error";
				return "the stream ends";
			}
		}
	}
	return action;
};
