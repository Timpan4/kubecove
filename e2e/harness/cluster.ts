export function kindConfig() {
	return `kind: Cluster\napiVersion: kind.x-k8s.io/v1alpha4\nnetworking:\n  disableDefaultCNI: true\nnodes:\n  - role: control-plane\n`;
}

export function kindDeleteArgs(cluster: string, kubeconfig: string) {
	if (!cluster || !kubeconfig) throw new Error("exact cluster cleanup requires cluster and kubeconfig");
	return ["delete", "cluster", "--name", cluster, "--kubeconfig", kubeconfig];
}
