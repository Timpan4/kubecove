export const fixturePaths = {
	all: "e2e/fixtures/all.yaml",
	ciliumValues: "e2e/fixtures/platform/cilium-values.yaml",
	argoCdValues: "e2e/fixtures/platform/argocd-values.yaml",
	traefikValues: "e2e/fixtures/platform/traefik-values.yaml",
	gitServer: "e2e/fixtures/platform/git-server.yaml",
	rootApplication: "e2e/fixtures/platform/root-application.yaml",
	rbac: "e2e/fixtures/rbac.yaml",
	gitops: "e2e/fixtures/gitops",
	chart: "e2e/fixtures/chart",
} as const;

export const chartPins = {
	cilium: { chart: "cilium", repository: "https://helm.cilium.io/cilium-1.19.6.tgz", version: "1.19.6", sha256: "21c43cf53841f9ab0375047d95aa4c64051ea52bbd2c679416e6408f5f1c9179" },
	argocd: { chart: "argo-cd", repository: "https://github.com/argoproj/argo-helm/releases/download/argo-cd-10.1.4/argo-cd-10.1.4.tgz", version: "10.1.4", appVersion: "3.4.5", sha256: "142d2eaaa2adf9051c109c396c5fe3af742674011a5837df262bd6f8f2991d2c" },
	metricsServer: { manifest: "https://github.com/kubernetes-sigs/metrics-server/releases/download/v0.9.0/components.yaml", version: "0.9.0", sha256: "1cec29a5267809306a2c6ec74a3e449abbb705b4a8beed0c8a1963910f72c79b" },
	localPath: { manifest: "https://raw.githubusercontent.com/rancher/local-path-provisioner/v0.0.36/deploy/local-path-storage.yaml", version: "0.0.36", sha256: "a5b4b057e4e400a2ca7188b03dc11303f874bfe600fc837d5446d86b3d13e26c" },
	traefik: { chart: "traefik", repository: "https://traefik.github.io/charts/traefik/traefik-41.0.2.tgz", version: "41.0.2", appVersion: "3.7.6", sha256: "71685966a482dfa2c2b39fedf7ae9b1391251314f87f4d05faa84e4848b8d3c2" },
} as const;

export const gitDaemonPins = {
	amd64: { version: "2.47.3-r0", url: "https://dl-cdn.alpinelinux.org/alpine/v3.21/main/x86_64/git-daemon-2.47.3-r0.apk", sha256: "06ce97d655dcb68bebbfb5355bdc2dd69c3020e38ff5fd2bb682776aefd44174" },
	arm64: { version: "2.47.3-r0", url: "https://dl-cdn.alpinelinux.org/alpine/v3.21/main/aarch64/git-daemon-2.47.3-r0.apk", sha256: "d072d4b3a5b7a79dff2bdec63ab771f63cfe27f464197a0bc45b540ec4132bab" },
} as const;

export function validateImmutablePins(pins = chartPins) {
	for (const pin of Object.values(pins)) {
		const source = "repository" in pin ? pin.repository : pin.manifest;
		if (!/^\d+\.\d+\.\d+$/.test(pin.version) || !/^https:\/\//.test(source) || !/^[a-f0-9]{64}$/.test(pin.sha256)) throw new Error("invalid immutable platform pin");
		if ("appVersion" in pin && !/^\d+\.\d+\.\d+$/.test(pin.appVersion)) throw new Error("invalid immutable app pin");
	}
	for (const pin of Object.values(gitDaemonPins)) if (!/^\d+\.\d+\.\d+-r\d+$/.test(pin.version) || !/^https:\/\//.test(pin.url) || !/^[a-f0-9]{64}$/.test(pin.sha256)) throw new Error("invalid immutable Git daemon pin");
}
