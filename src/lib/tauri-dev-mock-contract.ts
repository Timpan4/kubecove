import type { Channel, InvokeOptions } from "@tauri-apps/api/core";
import type { ArgoApplicationRef, ArgoOperationRequest, ArgoServerEndpoint } from "./gitops-types";
import type { ArgoManagedResource, DiscoveredResourceKind, FluxResourceKind, PodLogStreamRequest, ResourceListRequest, StreamMessage, TopologyMode, YamlApplyRequest } from "./types";

type MockRequest = Partial<ArgoOperationRequest & YamlApplyRequest & Pick<PodLogStreamRequest, "batchLines">>;
export interface MockArgs {
    cancelScope?: string;
    requestId?: string;
    content?: string;
    command?: string;
    args?: MockArgs;
	yamlEncoding?: YamlApplyRequest["yamlEncoding"];
	yamlViewMode?: "kubectl" | "applyClean";
	application?: Partial<ArgoApplicationRef>;
	channel?: Pick<Channel<StreamMessage>, "onmessage">;
	clusterContext?: string;
	endpoint?: ArgoServerEndpoint;
	envVar?: string;
	id?: string;
	kind?: string;
	kubeconfigEnvVar?: string;
	mode?: TopologyMode;
	name?: string;
	namespace?: string | null;
	namespaces?: string[];
	rememberCredential?: boolean;
	request?: MockRequest;
	requests?: ResourceListRequest[];
	resource?: ArgoManagedResource;
	resourceKind?: DiscoveredResourceKind | FluxResourceKind;
    keys?: import("./types").ResourceWatchStart[];
	serverUrl?: string;
	show?: boolean;
	storageName?: string;
    path?: string;
    paths?: string[];
	transport?: "connected" | "kubernetes";
	username?: string;
	workspaceId?: string;
}

export type MockResult = boolean | number | object | string | null | undefined;
export type MockHandler = (args: MockArgs, options?: InvokeOptions) => MockResult | Promise<MockResult>;
export function readMockArgs<Args extends object>(args: Args | undefined): MockArgs {
	if (args === undefined) return {};
	// SAFETY: browser mocks are called only through typed Tauri wrappers whose command arguments
	// match the corresponding handler fields declared by MockArgs.
	return args as MockArgs;
}
