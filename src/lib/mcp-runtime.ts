import { invoke } from "@tauri-apps/api/core";

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
	return invoke<McpStatus>("get_mcp_status");
}

export function setMcpSettings(enabled: boolean, port: number): Promise<McpStatus> {
	return invoke<McpStatus>("set_mcp_settings", { enabled, port });
}

export function revealMcpToken(): Promise<string> {
	return invoke<string>("reveal_mcp_token");
}

export function rotateMcpToken(): Promise<McpStatus> {
	return invoke<McpStatus>("rotate_mcp_token");
}

/** A Claude Code command that registers this endpoint. */
export function claudeCodeCommand(url: string, token: string): string {
	return `claude mcp add --transport http kubecove ${url} --header "Authorization: Bearer ${token}"`;
}
