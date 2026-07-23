export const safeDiagnosticCommands = [
	["get", "namespaces", "-o", "name"],
	["get", "nodes", "-o", "wide"],
	["get", "pods", "-A", "-o", "custom-columns=NAMESPACE:.metadata.namespace,NAME:.metadata.name,PHASE:.status.phase,RESTARTS:.status.containerStatuses[*].restartCount"],
	["get", "deployments", "-A", "-o", "wide"],
	["get", "daemonsets", "-A", "-o", "wide"],
	["get", "statefulsets", "-A", "-o", "wide"],
	["get", "persistentvolumeclaims", "-A", "-o", "wide"],
	["get", "persistentvolumes", "-o", "wide"],
	["get", "ingresses", "-A", "-o", "wide"],
	["get", "apiservice", "v1beta1.metrics.k8s.io", "-o", "custom-columns=NAME:.metadata.name,AVAILABLE:.status.conditions[?(@.type=='Available')].status"],
	["get", "applications.argoproj.io", "-n", "argocd", "-o", "wide"],
	["get", "applicationsets.argoproj.io", "-n", "argocd", "-o", "wide"],
	["get", "events", "-A", "--field-selector=type=Warning", "-o", "custom-columns=NAMESPACE:.metadata.namespace,REASON:.reason,OBJECT:.involvedObject.kind/NAME:.involvedObject.name"],
	["exec", "-n", "kube-system", "daemonset/cilium", "--", "cilium", "status", "--brief"],
	["logs", "-n", "argocd", "statefulset/argocd-application-controller", "--tail=200"],
	["logs", "-n", "argocd", "deployment/argocd-repo-server", "--tail=200"],
	["logs", "-n", "operations", "-l", "app.kubernetes.io/name=operations-crashloop", "--tail=20", "--prefix=true"],
] as const;

export function safeDiagnosticText(text: string) {
	return text
		.replace(/(token:\s*)\S+/gi, "$1REDACTED")
		.replace(/(password:\s*)\S+/gi, "$1REDACTED")
		.replace(/(authorization:\s*)\S+(?:\s+\S+)?/gi, "$1REDACTED")
		.replace(/(bearer\s+)\S+/gi, "$1REDACTED")
		.replace(/(client-certificate-data:\s*)\S+/gi, "$1REDACTED")
		.replace(/(client-key-data:\s*)\S+/gi, "$1REDACTED");
}
