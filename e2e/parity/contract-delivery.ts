import type { JsonObject } from "../../src/lib/types";
import { type StepParsers, boolean, count, noPayload, oneOf, string } from "./contract-values";

function gitOpsIdentity(item: JsonObject) {
	return { kind: string(item.kind), name: string(item.name), namespace: string(item.namespace) };
}

function helmReleaseIdentity(item: JsonObject) {
	return { name: string(item.name), namespace: string(item.namespace) };
}

export const deliverySteps = {
	expectGitOpsCount: (item) => ({ label: string(item.label), count: count(item.count) }),
	expectDeliveryView: (item) => ({ page: string(item.page), mode: string(item.mode) }),
	openGitOps: noPayload,
	openGitOpsResources: gitOpsIdentity,
	expectArgoWorkspace: (item) => ({ name: string(item.name) }),
	filterArgoResources: (item) => ({ filter: string(item.filter) }),
	setDeliveryView: (item) => ({
		page: oneOf(["GitOps", "Helm"], item.page, "a delivery page"),
		mode: oneOf(["Cards", "List"], item.mode, "Cards or List"),
	}),
	expectGitOpsTable: (item) => ({ title: string(item.title), count: count(item.count) }),
	expectGitOpsItem: gitOpsIdentity,
	expectNoGitOpsItem: gitOpsIdentity,
	openGitOpsDetails: gitOpsIdentity,
	expectGitOpsGroup: (item) => ({ label: string(item.label), position: count(item.position), count: count(item.count), expanded: boolean(item.expanded) }),
	toggleGitOpsGroup: (item) => ({ label: string(item.label) }),
	expectArgoApplication: (item) => ({ name: string(item.name), health: string(item.health), sync: string(item.sync) }),
	expectArgoSignal: (item) => ({ label: string(item.label), count: count(item.count) }),
	expectHelmCount: (item) => ({ count: count(item.count) }),
	expectHelmRelease: helmReleaseIdentity,
	expectNoHelmRelease: helmReleaseIdentity,
	openHelmRelease: helmReleaseIdentity,
	selectHelmNamespace: (item) => ({ namespace: string(item.namespace) }),
	searchHelm: (item) => ({ value: string(item.value) }),
	expectHelmDetails: (item) => ({ title: string(item.title), description: string(item.description), totals: string(item.totals) }),
	expectHelmContext: (item) => ({ context: string(item.context) }),
} satisfies StepParsers;
