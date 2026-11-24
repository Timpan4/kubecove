// Keep the build flag and imports in the same expression so Vite can omit these chunks.
export const browserMockSurfaces = process.env.KUBECOVE_PUBLIC_DEV_MOCKS === "true"
	? {
		appEffects: () => import("./BrowserMockAppEffects.svelte"),
		updateButton: () => import("./UpdateStatusButton.svelte"),
		updateSettings: () => import("./UpdatesSettings.svelte"),
		remoteSettings: () => import("./RemoteConnectionSettings.svelte"),
		remoteHostScope: () => import("@/features/remote-connection/RemoteHostScope.svelte"),
		remoteNotice: () => import("@/features/remote-connection/RemoteWorkspaceNotice.svelte"),
	}
	: undefined;
