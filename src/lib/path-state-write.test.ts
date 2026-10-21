import { afterEach, expect, test } from "bun:test";
import {
	defaultPathStateSnapshot,
	flushPathStateWrite,
	readPathState,
	schedulePathStateWrite,
	writePathState,
} from "./path-state";

const originalWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
const originalRequestFrame = globalThis.requestAnimationFrame;
const originalCancelFrame = globalThis.cancelAnimationFrame;
const originalDocument = Object.getOwnPropertyDescriptor(globalThis, "document");
afterEach(() => {
	flushPathStateWrite();
	if (originalWindow)
		Object.defineProperty(globalThis, "window", originalWindow);
	else Reflect.deleteProperty(globalThis, "window");
	globalThis.requestAnimationFrame = originalRequestFrame;
	globalThis.cancelAnimationFrame = originalCancelFrame;
	if (originalDocument)
		Object.defineProperty(globalThis, "document", originalDocument);
	else Reflect.deleteProperty(globalThis, "document");
});

// Failure modes: each keystroke serializes, a pending write is lost on unload,
// or an older scheduled workspace snapshot overwrites explicit navigation.
test("path persistence coalesces changes, flushes the latest once, and yields to immediate navigation", () => {
	const frames = new Map<number, FrameRequestCallback>();
	let frameId = 0;
	globalThis.requestAnimationFrame = (callback) => {
		frames.set(++frameId, callback);
		return frameId;
	};
	globalThis.cancelAnimationFrame = (id) => { frames.delete(id); };
	const paint = () => {
		for (const [id, callback] of frames) {
			frames.delete(id);
			callback(0);
		}
	};
	const stored = new Map<string, string>();
	let writes = 0;
	const windowMock = {
		location: { hash: "" },
		history: { replaceState() {} },
		sessionStorage: {
			getItem: (key: string) => stored.get(key) ?? null,
			setItem: (key: string, value: string) => {
				writes++;
				stored.set(key, value);
			},
		},
	};
	Object.defineProperty(globalThis, "window", {
		configurable: true,
		value: windowMock,
	});
	schedulePathStateWrite(defaultPathStateSnapshot("workspaces"));
	schedulePathStateWrite(defaultPathStateSnapshot("settings"));
	expect(writes).toBe(0);
	expect(frames.size).toBe(1);
	paint();
	expect(writes).toBe(1);
	expect(readPathState()?.launcherView).toBe("settings");
	schedulePathStateWrite(defaultPathStateSnapshot("workspaces"));
	flushPathStateWrite();
	flushPathStateWrite();
	expect(writes).toBe(2);
	expect(frames.size).toBe(0);
	schedulePathStateWrite(defaultPathStateSnapshot("settings"));
	writePathState(defaultPathStateSnapshot("workspaces"));
	paint();
	expect(writes).toBe(3);
	expect(readPathState()?.launcherView).toBe("workspaces");
	Object.defineProperty(globalThis, "document", {
		configurable: true,
		value: { visibilityState: "hidden" },
	});
	schedulePathStateWrite(defaultPathStateSnapshot("settings"));
	expect(frames.size).toBe(0);
	expect(writes).toBe(4);
	expect(readPathState()?.launcherView).toBe("settings");
});
