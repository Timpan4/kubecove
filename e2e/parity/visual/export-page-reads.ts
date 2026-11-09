import { pageReadResult, replayPageReads } from "../../../src/lib/tauri-dev-mock-pages";

export function exportPageReads<Responses extends object>(responses: Responses) {
    replayPageReads("updated");
    try {
        return Object.fromEntries(Object.entries(responses).map(([command, value]) => [command, pageReadResult(command, value)]));
    } finally { replayPageReads("base"); }
}
