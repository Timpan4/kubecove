import type { McpStatus } from "./mcp-runtime";

// Controlled credentials, never a host token. This module does not open sockets.
export const MCP_FIXTURE_TOKEN = "parity-mcp-credential";
export const MCP_ROTATED_TOKEN = "parity-mcp-rotated-credential";
const tools = ["list_contexts", "list_namespaces", "list_resource_kinds", "list_resources", "find_problems", "describe_resource", "get_resource_yaml", "get_pod_logs", "get_resource_metrics", "list_argocd_applications", "list_helm_releases"];
let status: McpStatus = { enabled: false, port: 47615, running: false, url: null, error: null, tools };
let rotated = false;
const requests: string[] = [];
export function mcpMockRequests(): string[] { return [...requests]; }
export function mcpMockStatus(): McpStatus { return { ...status, tools: [...tools] }; }
export function mcpMockInvoke(command: string, args?: { enabled: boolean; port: number }): McpStatus | string {
    switch (command) {
        case "get_mcp_status": requests.push("status"); return mcpMockStatus();
        case "set_mcp_settings":
            if (!args || !Number.isInteger(args.port) || args.port < 1 || args.port > 65535) throw new Error("Invalid MCP port");
            requests.push(`set:${args.enabled}:${args.port}`);
            status = { ...status, ...args, running: args.enabled, url: args.enabled ? `http://127.0.0.1:${args.port}/mcp` : null };
            return mcpMockStatus();
        case "reveal_mcp_token": requests.push("reveal"); return rotated ? MCP_ROTATED_TOKEN : MCP_FIXTURE_TOKEN;
        case "rotate_mcp_token": requests.push("rotate"); rotated = true; return mcpMockStatus();
        default: throw new Error("Unknown MCP fixture command");
    }
}
