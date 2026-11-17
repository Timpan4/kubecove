import type { McpStatus } from "./mcp-runtime";

// Controlled credentials, never a host token. This module does not open sockets.
export const MCP_FIXTURE_TOKEN = "parity-mcp-credential";
export const MCP_ROTATED_TOKEN = "parity-mcp-rotated-credential";
const tools = ["list_contexts", "list_namespaces", "list_resource_kinds", "list_resources", "find_problems", "describe_resource", "get_resource_yaml", "get_pod_logs", "get_resource_metrics", "list_argocd_applications", "list_helm_releases"];
let status: McpStatus = { enabled: false, port: 47615, running: false, url: null, error: null, tools };
let rotated = false;
/** The settings file as the host last saw it. Mirrors `McpFixture` in the GPUI tests. */
let settingsFile: "loaded" | "invalid" | "fixed" = "loaded";
const requests: string[] = [];
export function mcpMockRequests(): string[] { return [...requests]; }
export function mcpMockStatus(): McpStatus { return { ...status, tools: [...tools] }; }
/** `invalid` stands for a file the host could not load at startup; `valid` for the same file after the user fixes it. */
export function configureMcpSettingsFile(file: "invalid" | "valid"): void {
    if (file === "invalid") settingsFile = "invalid";
    else if (settingsFile === "invalid") settingsFile = "fixed";
}
/** Logs the request and, like the host, refuses it before any change while the settings file is not loaded. */
function request(name: string): void {
    requests.push(name);
    if (settingsFile !== "loaded") throw new Error("Invalid MCP settings file");
}
export function mcpMockInvoke(command: string, args?: { enabled: boolean; port: number }): McpStatus | string {
    switch (command) {
        case "get_mcp_status": request("status"); return mcpMockStatus();
        case "set_mcp_settings":
            if (!args || !Number.isInteger(args.port) || args.port < 1 || args.port > 65535) throw new Error("Invalid MCP port");
            request(`set:${args.enabled}:${args.port}`);
            status = { ...status, ...args, running: args.enabled, url: args.enabled ? `http://127.0.0.1:${args.port}/mcp` : null };
            return mcpMockStatus();
        case "reload_mcp_settings":
            if (settingsFile === "fixed") settingsFile = "loaded";
            request("reload");
            return mcpMockStatus();
        case "reveal_mcp_token": request("reveal"); return rotated ? MCP_ROTATED_TOKEN : MCP_FIXTURE_TOKEN;
        case "rotate_mcp_token": request("rotate"); rotated = true; return mcpMockStatus();
        default: throw new Error("Unknown MCP fixture command");
    }
}
