/// <reference types="vite/client" />

interface ImportMetaEnv {
	readonly VITE_KUBECOVE_RELEASE_CHANNEL?: "stable" | "dev";
	readonly VITE_KUBECOVE_REACT_COMPILER_ENABLED?: "true" | "false";
}
