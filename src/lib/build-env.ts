export const IS_DEV_BUILD = process.env.KUBECOVE_PUBLIC_DEV === "true";

export const RELEASE_CHANNEL =
	process.env.KUBECOVE_PUBLIC_RELEASE_CHANNEL === "stable" ? "stable" : "dev";
