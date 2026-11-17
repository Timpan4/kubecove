import { type StepParsers, boolean, count, string, noPayload } from "./contract-values";

export const deliverySteps = {
	expectGitOpsCount: (item) => ({ label: string(item.label), count: count(item.count) }),
	expectDeliveryView: (item) => ({ page: string(item.page), mode: string(item.mode) }),
	openGitOps: noPayload,
	openGitOpsResources: (item) => ({ kind: string(item.kind), name: string(item.name), namespace: string(item.namespace) }),
	expectArgoWorkspace: (item) => ({ name: string(item.name) }),
	filterArgoResources: (item) => ({ filter: string(item.filter) }),
	setDeliveryView: (item) => {
		if (item.page !== "GitOps" && item.page !== "Helm") throw new Error("Expected a delivery page");
		if (item.mode !== "Cards" && item.mode !== "List") throw new Error("Expected Cards or List");
		return { page: item.page, mode: item.mode };
	},
	expectGitOpsTable: (item) => ({ title: string(item.title), count: count(item.count) }),
	expectGitOpsItem: (item) => ({ kind: string(item.kind), name: string(item.name), namespace: string(item.namespace) }),
	expectNoGitOpsItem: (item) => ({ kind: string(item.kind), name: string(item.name), namespace: string(item.namespace) }),
	openGitOpsDetails: (item) => ({ kind: string(item.kind), name: string(item.name), namespace: string(item.namespace) }),
	expectGitOpsGroup: (item) => ({ label: string(item.label), position: count(item.position), count: count(item.count), expanded: boolean(item.expanded) }),
	toggleGitOpsGroup: (item) => ({ label: string(item.label) }),
	expectArgoApplication: (item) => ({ name: string(item.name), health: string(item.health), sync: string(item.sync) }),
	expectArgoSignal: (item) => ({ label: string(item.label), count: count(item.count) }),
	expectHelmCount: (item) => ({ count: count(item.count) }),
	expectHelmRelease: (item) => ({ name: string(item.name), namespace: string(item.namespace) }),
	expectNoHelmRelease: (item) => ({ name: string(item.name), namespace: string(item.namespace) }),
	openHelmRelease: (item) => ({ name: string(item.name), namespace: string(item.namespace) }),
	selectHelmNamespace: (item) => ({ namespace: string(item.namespace) }),
	searchHelm: (item) => ({ value: string(item.value) }),
	expectHelmDetails: (item) => ({ title: string(item.title), description: string(item.description), totals: string(item.totals) }),
	expectHelmContext: (item) => ({ context: string(item.context) }),
} satisfies StepParsers;
