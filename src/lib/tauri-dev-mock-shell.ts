export type ShellReadPhase = "ready" | "contexts-failed" | "contexts-empty" | "namespaces-failed" | "namespaces-empty" | "resources-failed";
function initialPhase(): ShellReadPhase {
    const value = globalThis.sessionStorage?.getItem("parity-shell-read");
    switch (value) {
        case "contexts-failed": case "contexts-empty": case "namespaces-failed": case "namespaces-empty": case "resources-failed": return value;
        default: return "ready";
    }
}
let phase: ShellReadPhase = initialPhase();

export function configureShellRead(next: ShellReadPhase): void {
    phase = next;
    globalThis.sessionStorage?.setItem("parity-shell-read", next);
}

export function shellRead<Item>(read: "contexts" | "namespaces" | "resources", items: Item[]): Item[] {
    if (phase === `${read}-failed`) throw new Error(`Scripted ${read} read failure`);
    const empty = (read === "contexts" && phase === "contexts-empty") || (read === "namespaces" && phase === "namespaces-empty");
    return empty ? [] : items;
}
