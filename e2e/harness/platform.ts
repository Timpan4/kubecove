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
	cilium: { chart: "cilium", repository: "https://helm.cilium.io/cilium-1.20.3.tgz", version: "1.20.3", sha256: "0805a5273118c436c9772bb9566d87ec558d8b25cc2ce5dc32918ecdd36f2c7c" },
	argocd: { chart: "argo-cd", repository: "https://github.com/argoproj/argo-helm/releases/download/argo-cd-10.10.5/argo-cd-10.10.5.tgz", version: "10.10.5", appVersion: "3.5.5", sha256: "ba6447eeed42bc41d1f0527721047b236304e97669a7b6286d02be4a1644ba7a" },
	metricsServer: { manifest: "https://github.com/kubernetes-sigs/metrics-server/releases/download/v0.9.0/components.yaml", version: "0.9.0", sha256: "1cec29a5267809306a2c6ec74a3e449abbb705b4a8beed0c8a1963910f72c79b" },
	localPath: { manifest: "https://raw.githubusercontent.com/rancher/local-path-provisioner/v0.0.37/deploy/local-path-storage.yaml", version: "0.0.37", sha256: "9781b39c24f3f651bd6d6e41b561e04e4904bbdb6d4f8c7a6009df3a702dcd65" },
	traefik: { chart: "traefik", repository: "https://traefik.github.io/charts/traefik/traefik-42.1.2.tgz", version: "42.1.2", appVersion: "3.7.16", sha256: "ecf9ac8bc0837e3260722566c2a49919332ca96b0b4e962405dfcfae51838699" },
} as const;

export const gitDaemonPins = {
	amd64: { version: "2.54.0-r0", url: "https://dl-cdn.alpinelinux.org/alpine/v3.24/main/x86_64/git-daemon-2.54.0-r0.apk", sha256: "b376522eadb4df8dc5274687ceabe07456924d415cfcc5683c4b198e97a6c855" },
	arm64: { version: "2.54.0-r0", url: "https://dl-cdn.alpinelinux.org/alpine/v3.24/main/aarch64/git-daemon-2.54.0-r0.apk", sha256: "e34fc8841226216b798b8e1ff20c2ad856d97c2bc2c512f6b7e183d934f3d9b0" },
} as const;

export function validateImmutablePins(pins = chartPins) {
	for (const pin of Object.values(pins)) {
		const source = "repository" in pin ? pin.repository : pin.manifest;
		if (!/^\d+\.\d+\.\d+$/.test(pin.version) || !source.startsWith('https://') || !/^[a-f0-9]{64}$/.test(pin.sha256)) throw new Error("invalid immutable platform pin");
		if ("appVersion" in pin && !/^\d+\.\d+\.\d+$/.test(pin.appVersion)) throw new Error("invalid immutable app pin");
	}
	for (const pin of Object.values(gitDaemonPins)) if (!/^\d+\.\d+\.\d+-r\d+$/.test(pin.version) || !pin.url.startsWith('https://') || !/^[a-f0-9]{64}$/.test(pin.sha256)) throw new Error("invalid immutable Git daemon pin");
}
