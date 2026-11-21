async function invokeMcpMock<T>(command: string, args?: { enabled: boolean; port: number }): Promise<T> {
	if (process.env.KUBECOVE_PUBLIC_DEV_MOCKS === "true") {
		const { mcpMockInvoke } = await import("./mcp-dev-mocks");
		// SAFETY: these wrappers own the result contract of each mock command.
		return mcpMockInvoke(command, args) as T;
	}
	throw new Error("Desktop MCP settings are only available in browser mocks.");
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

// These mocks model desktop MCP controls independently of any selected remote.
export function getMcpStatus(): Promise<McpStatus> {
	return invokeMcpMock<McpStatus>("get_mcp_status");
}

export function setMcpSettings(enabled: boolean, port: number): Promise<McpStatus> {
	return invokeMcpMock<McpStatus>("set_mcp_settings", { enabled, port });
}

/** Re-reads the settings file after the host failed to load it. */
export function reloadMcpSettings(): Promise<McpStatus> {
	return invokeMcpMock<McpStatus>("reload_mcp_settings");
}

export function revealMcpToken(): Promise<string> {
	return invokeMcpMock<string>("reveal_mcp_token");
}

export function rotateMcpToken(): Promise<McpStatus> {
	return invokeMcpMock<McpStatus>("rotate_mcp_token");
}

/** A Claude Code command that registers this endpoint. */
export function claudeCodeCommand(url: string, token: string): string {
	return `claude mcp add --transport http kubecove ${url} --header "Authorization: Bearer ${token}"`;
}
