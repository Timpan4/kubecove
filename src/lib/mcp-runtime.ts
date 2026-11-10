import { invoke } from "@tauri-apps/api/core";
import { isBrowserDevMockMode } from "./tauri-runtime";

// MCP stays local and bypasses the remote client and result diagnostics.
async function localInvoke<T>(command: string, args?: { enabled: boolean; port: number }): Promise<T> {
    if (isBrowserDevMockMode()) {
        const { mcpMockInvoke } = await import("./mcp-dev-mocks");
        // SAFETY: these desktop wrappers own the result contract of each mock command.
        return mcpMockInvoke(command, args) as T;
    }
    return invoke<T>(command, args);
}

/** The desktop app's loopback MCP listener (ADR 0028). Mirrors `McpStatus` in Rust. */
export interface McpStatus {
	enabled: boolean;
	port: number;
	running: boolean;
	url: string | null;
	error: string | null;
	tools: string[];
}

// These commands are desktop-only and always act on this machine, so they bypass the
// remote-server client even while a remote server is selected.
export function getMcpStatus(): Promise<McpStatus> {
	return localInvoke<McpStatus>("get_mcp_status");
}

export function setMcpSettings(enabled: boolean, port: number): Promise<McpStatus> {
	return localInvoke<McpStatus>("set_mcp_settings", { enabled, port });
}

export function revealMcpToken(): Promise<string> {
	return localInvoke<string>("reveal_mcp_token");
}

export function rotateMcpToken(): Promise<McpStatus> {
	return localInvoke<McpStatus>("rotate_mcp_token");
}

/** A Claude Code command that registers this endpoint. */
export function claudeCodeCommand(url: string, token: string): string {
	return `claude mcp add --transport http kubecove ${url} --header "Authorization: Bearer ${token}"`;
}
