// Presentation labels preserve original evidence in the expanded entry.
import type { IncidentTimelineItem } from "./incident-timeline";

export function incidentTimelineTitle(item: IncidentTimelineItem, resourceKind: string): string {
	const detail = item.detail ?? "";
	const storage = /low on resource:\s*ephemeral-storage/i.test(detail);
	const probe = detail.match(/\b(startup|readiness|liveness) probe/i)?.[1];
	if (item.source === "event" && probe) {
		const name = `${probe[0].toUpperCase()}${probe.slice(1).toLowerCase()} probe`;
		if (/CONTAINER_EXITED/i.test(detail)) return `${name} could not run: container exited`;
		if (/timed out/i.test(detail)) {
			const duration = detail.match(/timed out after\s+([\d.]+\s*(?:ms|s|seconds?)\b)/i)?.[1];
			return `${name} timed out${duration ? ` after ${duration}` : ""}`;
		}
		if (/errored/i.test(detail)) return `${name} could not run`;
		if (/failed/i.test(detail)) return `${name} failed`;
	}
	if (item.title === "Warning FailedScheduling") {
		if (/untolerated taint/i.test(detail)) return "Scheduling blocked: node taint not tolerated";
		const resource = detail.match(/Insufficient\s+([\w-]+)/i)?.[1];
		if (resource) return `Scheduling blocked: insufficient ${resource}`;
		return detail || "Pod could not be scheduled";
	}
	if (item.title === "Warning Evicted") {
		return storage ? "Pod evicted: node low on ephemeral storage" : detail || "Pod evicted";
	}
	if (item.title === "Warning BackOff") {
		const container = detail.match(/restarting failed container\s+(\S+)/i)?.[1];
		return container ? `${container} restart delayed after failure` : detail || "Container restart delayed";
	}
	if (item.source === "condition") {
		const failed = detail.includes("PodFailed") ? ": pod failed" : "";
		if (item.title === "ContainersReady=False") return `Containers not ready${failed}`;
		if (item.title === "Ready=False") return `${resourceKind} not ready${failed}`;
		if (item.title === "PodReadyToStartContainers=False") return "Pod not ready to start containers";
		if (item.title === "DisruptionTarget=True") {
			if (storage) return "Pod marked for termination: node low on ephemeral storage";
			if (detail.includes("TerminationByKubelet")) return "Pod marked for termination by kubelet";
			return "Pod marked for disruption";
		}
	}
	if (item.source === "restart") {
		const count = detail.match(/^(\d+) restarts?\b/)?.[1];
		if (count) return `${item.title} ${count} ${count === "1" ? "time" : "times"}`;
	}
	if (item.source === "container") {
		const code = detail.match(/\bexit (-?\d+)\b/)?.[1];
		if (code) return `${item.title} with exit code ${code}`;
	}
	if (item.source === "container" || item.source === "restart" || item.source === "condition") return item.title;
	return detail || item.title.replace(/^Warning\s+/, "");
}
