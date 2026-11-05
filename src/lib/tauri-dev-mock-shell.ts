export type ShellReadPhase = "ready" | "contexts-failed" | "contexts-empty" | "namespaces-failed" | "resources-failed";
function initialPhase(): ShellReadPhase {
    const value = globalThis.sessionStorage?.getItem("parity-shell-read");
    switch (value) {
        case "contexts-failed": case "contexts-empty": case "namespaces-failed": case "resources-failed": return value;
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
    return read === "contexts" && phase === "contexts-empty" ? [] : items;
}
