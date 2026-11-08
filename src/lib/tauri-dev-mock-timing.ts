import type { MockArgs } from "./tauri-dev-mock-contract";

export type TimingControl = {
    operation: "hold" | "dirty" | "contend" | "release" | "fail" | "failOnce" | "expectRequests" | "expectPending" | "expectNoRetry" | "expectNoError";
    command: string;
    context: string;
    namespace?: string;
    kind?: string;
    count?: number;
};

class MockReadError extends Error {
    constructor(readonly kind: string) { super(`Scripted read: ${kind}`); }
}

type Load = {
    dirty: boolean;
    waiters: number;
    contend: () => void;
    promise: Promise<unknown>;
    resolve: () => void;
    reject: (error: Error) => void;
};
type Rule = { held: boolean; requests: number; failedAt?: number; failure?: Error; loads: Map<string, Load> };

/** Explicit release gates model overlap without introducing wall-clock latency values. */
export class MockTiming {
    private readonly rules = new Map<string, Rule>();
    private readonly scopes = new Map<string, Map<string, AbortController>>();

    requestCount(command: string, context: string): number {
        return this.rules.get(this.key(command, context))?.requests ?? 0;
    }
    pending(command: string, context: string, namespace?: string): boolean {
        const loads = this.rules.get(this.key(command, context))?.loads;
        return Boolean(loads && [...loads.keys()].some((key) => namespace === undefined || JSON.parse(key)[0] === namespace));
    }

    control(control: TimingControl): void {
        const key = this.key(control.command, control.context);
        const rule = this.rules.get(key) ?? { held: false, requests: 0, loads: new Map<string, Load>() };
        this.rules.set(key, rule);
        switch (control.operation) {
            case "hold": rule.held = true; break;
            case "dirty": for (const load of rule.loads.values()) load.dirty = true; break;
            case "contend": {
                // Starting a competitor can replace entries in this map.
                const pending = Array.from(rule.loads.values());
                for (const load of pending) load.contend();
                break;
            }
            case "release":
                rule.held = false;
                rule.failure = undefined;
                for (const [key, load] of rule.loads) if (control.namespace === undefined || JSON.parse(key)[0] === control.namespace) load.resolve();
                break;
            case "fail":
            case "failOnce":
                rule.held = false;
                rule.failedAt = rule.requests;
                rule.failure = control.operation === "fail" ? new MockReadError(control.kind ?? "transport") : undefined;
                for (const [key, load] of rule.loads) if (control.namespace === undefined || JSON.parse(key)[0] === control.namespace) load.reject(new MockReadError(control.kind ?? "transport"));
                break;
            case "expectNoError": break;
            case "expectNoRetry":
                if (rule.requests !== rule.failedAt) throw new Error(`Read retried after failure for ${key}`);
                break;
            case "expectRequests":
                if (rule.requests !== control.count) throw new Error(`Expected ${control.count} requests for ${key}, received ${rule.requests}`);
                break;
            case "expectPending":
                if (!this.pending(control.command, control.context, control.namespace)) throw new Error(`No pending read for ${key}`);
                break;
        }
    }

    cancelScope(scope: string): number {
        const requests = this.scopes.get(scope.trim());
        this.scopes.delete(scope.trim());
        for (const request of requests?.values() ?? []) request.abort(new MockReadError("cancelled"));
        return requests?.size ?? 0;
    }

    cancelAll(): number {
        let cancelled = 0;
        for (const scope of this.scopes.keys()) cancelled += this.cancelScope(scope);
        for (const rule of this.rules.values()) {
            for (const load of rule.loads.values()) load.reject(new MockReadError("cancelled"));
            rule.loads.clear();
        }
        return cancelled;
    }

    async read<T>(command: string, args: MockArgs, read: () => T | Promise<T>): Promise<T> {
        // cancellation.rs registers only when both normalized identifiers exist.
        const scope = args.cancelScope?.trim();
        const id = args.requestId?.trim();
        if (!scope || !id) return this.authenticatedRead(command, args, read);
        const requests = this.scopes.get(scope) ?? new Map<string, AbortController>();
        this.scopes.set(scope, requests);
        requests.get(id)?.abort(new MockReadError("cancelled"));
        const controller = new AbortController();
        requests.set(id, controller);
        const cancelled = Promise.withResolvers<never>();
        const abort = () => cancelled.reject(controller.signal.reason);
        controller.signal.addEventListener("abort", abort, { once: true });
        try {
            return await Promise.race([this.authenticatedRead(command, args, read, controller.signal), cancelled.promise]);
        } finally {
            controller.signal.removeEventListener("abort", abort);
            // A stale completion must not unregister its replacement.
            if (requests.get(id) === controller) requests.delete(id);
            if (!requests.size && this.scopes.get(scope) === requests) this.scopes.delete(scope);
        }
    }

    private async authenticatedRead<T>(command: string, args: MockArgs, read: () => T | Promise<T>, signal?: AbortSignal): Promise<T> {
        if (command !== "credential" && this.rules.get(this.key("credential", args.clusterContext ?? ""))?.held) {
            // The finite-client credential resolution is shared per context.
            await this.load("credential", { clusterContext: args.clusterContext }, () => undefined, signal);
        }
        signal?.throwIfAborted();
        if (command === "start_resource_watch") {
            // A delayed watch must not register its channel before it opens.
            await this.load(command, args, () => undefined, signal);
            signal?.throwIfAborted();
            return read();
        }
        return this.load(command, args, read, signal);
    }

    invalidate(context: string): void {
        for (const [key, rule] of this.rules) {
            // Stream opening has no finite snapshot for a resource update to dirty.
            if (key === this.key("credential", context) || key === this.key("start_resource_watch", context)) continue;
            if (JSON.parse(key)[1] === context) for (const load of rule.loads.values()) load.dirty = true;
        }
    }

    private load<T>(command: string, args: MockArgs, read: () => T | Promise<T>, signal?: AbortSignal): Promise<T> {
        const rule = this.rules.get(this.key(command, args.clusterContext ?? args.request?.clusterContext ?? ""));
        if (!rule) return Promise.resolve().then(read);
        rule.requests++;
        if (rule.failure) return Promise.reject(rule.failure);
        if (!rule.held) return Promise.resolve().then(read);
        const namespace = args.namespace ?? args.requests?.[0]?.namespace ?? (args.namespaces?.length === 1 ? args.namespaces[0] : null);
        const key = JSON.stringify([namespace, args.namespaces?.toSorted(), args.kind, args.resourceKind, args.mode, args.name, args.requests, args.kubeconfigEnvVar]);
        let load = rule.loads.get(key);
        // Mirrors SharedCache::get_or_load in commands/live_store.rs. A clean
        // load joins; the next caller after invalidation cancels the old load.
        if (load?.dirty) {
            load.reject(new MockReadError("cancelled"));
            rule.loads.delete(key);
            load = undefined;
        }
        if (!load) {
            const gate = Promise.withResolvers<void>();
            const snapshot = Promise.resolve().then(read);
            const pending: Load = {
                dirty: false,
                waiters: 0,
                // A second backend consumer has its own lifetime and consumes its result.
                contend: () => { void this.load(command, args, read).catch(() => {}); },
                promise: Promise.all([gate.promise, snapshot]).then(([, value]) => value),
                resolve: () => gate.resolve(),
                reject: gate.reject,
            };
            load = pending;
            rule.loads.set(key, pending);
            void pending.promise.finally(() => {
                if (rule.loads.get(key) === pending) rule.loads.delete(key);
            }).catch(() => {});
        }
        load.waiters++;
        const active = load;
        const cancellation = Promise.withResolvers<never>();
        const abort = () => cancellation.reject(signal?.reason);
        signal?.addEventListener("abort", abort, { once: true });
        const result = Promise.race([active.promise, cancellation.promise]).finally(() => {
            signal?.removeEventListener("abort", abort);
            active.waiters--;
            if (!active.waiters && rule.loads.get(key) === active) {
                rule.loads.delete(key);
                active.reject(new MockReadError("cancelled"));
            }
        });
        // SAFETY: this load is only joined by callers with the same command and request key.
        return result as Promise<T>;
    }

    private key(command: string, context: string): string { return JSON.stringify([command, context]); }
}

export const mockTiming = new MockTiming();

/** Holds the parity web driver stored before a reload, applied before the reloaded app's first read. */
export const STARTUP_HOLDS_KEY = "parity-startup-holds";
const startupHolds = globalThis.sessionStorage?.getItem(STARTUP_HOLDS_KEY);
if (startupHolds) {
    globalThis.sessionStorage.removeItem(STARTUP_HOLDS_KEY);
    for (const control of JSON.parse(startupHolds) as TimingControl[]) if (control.operation === "hold") mockTiming.control(control);
}
