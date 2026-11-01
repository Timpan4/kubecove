export interface ResourceLifecycle {
	label: string;
	progress: boolean;
	tone: "info" | "warning" | "error";
}

const WORKLOADS = new Set(["Pod", "Deployment", "ReplicaSet", "StatefulSet", "DaemonSet"]);
const FAILURES = new Set([
	"ImagePullBackOff", "ErrImagePull", "CrashLoopBackOff", "CreateContainerConfigError",
	"CreateContainerError", "RunContainerError", "InvalidImageName", "Rollout failed",
]);

export function resourceLifecycle(kind: string, status: string | null | undefined): ResourceLifecycle | null {
	if (!status || !WORKLOADS.has(kind)) return null;
	if (FAILURES.has(status)) return { label: status, progress: false, tone: "error" };
	if (status === "Paused") return { label: status, progress: false, tone: "warning" };
	if (status === "Rolling out" || status === "Starting" || status === "Terminating" || status === "Scaling down") {
		return { label: status, progress: true, tone: "info" };
	}
	if (kind === "Pod" && (status === "Pending" || status === "ContainerCreating" || status === "PodInitializing")) {
		return { label: "Starting", progress: true, tone: "info" };
	}
	return null;
}
