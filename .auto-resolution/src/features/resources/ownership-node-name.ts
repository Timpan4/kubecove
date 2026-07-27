export function smartKubernetesName(name: string, kind: string): string {
	if (name.length <= 28) return name;
	const parts = name.split("-");
	const suffix = parts[parts.length - 1] ?? "";
	const generatedPodName =
		kind === "Pod" &&
		parts.length >= 3 &&
		/^[a-z0-9]{4,6}$/i.test(suffix);
	if (generatedPodName) {
		const prefix = parts.slice(0, -2).join("-");
		if (prefix.length >= 4) return `${prefix}...${suffix}`;
	}
	const generatedControllerName =
		["ReplicaSet", "Job"].includes(kind) &&
		parts.length >= 2 &&
		/^[a-z0-9]{6,12}$/i.test(suffix);
	if (generatedControllerName) {
		const prefix = parts.slice(0, -1).join("-");
		if (prefix.length >= 4) return `${prefix}-${suffix.slice(0, 3)}...`;
	}
	const headLength = name.length > 28 ? 15 : 14;
	const tailLength = name.length > 28 ? 5 : 4;
	return `${name.slice(0, headLength)}...${name.slice(-tailLength)}`;
}
