import type { NodeTypes } from "@xyflow/react";
import {
	OwnershipResourceNode,
	StandaloneKindGroupNode,
} from "./OwnershipMapNodeComponents";

export const ownershipMapNodeTypes = {
	ownershipResource: OwnershipResourceNode,
	standaloneKindGroup: StandaloneKindGroupNode,
} satisfies NodeTypes;
