import {
	Box,
	Boxes,
	BriefcaseBusiness,
	Clock3,
	Database,
	FileCog,
	FolderTree,
	GitBranch,
	Globe2,
	HardDrive,
	KeyRound,
	Layers,
	Network,
	Package,
	Rocket,
	Server,
	Shield,
	Workflow,
	type LucideIcon,
} from "lucide-react";

export interface ResourceVisual {
	icon: LucideIcon;
	className: string;
}

const WORKLOAD_VISUAL: ResourceVisual = {
	icon: Boxes,
	className: "text-[var(--resource-workload)]",
};

const NETWORK_VISUAL: ResourceVisual = {
	icon: Network,
	className: "text-[var(--resource-network)]",
};

const CONFIG_VISUAL: ResourceVisual = {
	icon: FileCog,
	className: "text-[var(--resource-config)]",
};

const STORAGE_VISUAL: ResourceVisual = {
	icon: HardDrive,
	className: "text-[var(--resource-storage)]",
};

const ARGO_VISUAL: ResourceVisual = {
	icon: GitBranch,
	className: "text-[var(--resource-argo)]",
};

const DEFAULT_VISUAL: ResourceVisual = {
	icon: Package,
	className: "text-muted-foreground",
};

const KIND_VISUALS: Record<string, ResourceVisual> = {
	Pod: { icon: Box, className: "text-[var(--resource-workload)]" },
	Deployment: { icon: Rocket, className: "text-[var(--resource-workload)]" },
	StatefulSet: { icon: Database, className: "text-[var(--resource-storage)]" },
	DaemonSet: { icon: Server, className: "text-[var(--resource-workload)]" },
	Job: { icon: BriefcaseBusiness, className: "text-[var(--resource-cluster)]" },
	CronJob: { icon: Clock3, className: "text-[var(--resource-cluster)]" },
	Service: { icon: Network, className: "text-[var(--resource-network)]" },
	Ingress: { icon: Globe2, className: "text-[var(--resource-network)]" },
	ConfigMap: { icon: FileCog, className: "text-[var(--resource-config)]" },
	Secret: { icon: KeyRound, className: "text-[var(--resource-secret)]" },
	PersistentVolumeClaim: { icon: HardDrive, className: "text-[var(--resource-storage)]" },
	PersistentVolume: { icon: Database, className: "text-[var(--resource-storage)]" },
	StorageClass: { icon: Layers, className: "text-[var(--resource-storage)]" },
	Node: { icon: Server, className: "text-[var(--resource-cluster)]" },
	Applications: { icon: GitBranch, className: "text-[var(--resource-argo)]" },
	Application: { icon: GitBranch, className: "text-[var(--resource-argo)]" },
	ApplicationSets: { icon: Workflow, className: "text-[var(--resource-argo)]" },
	ApplicationSet: { icon: Workflow, className: "text-[var(--resource-argo)]" },
	AppProjects: { icon: Shield, className: "text-[var(--resource-argo)]" },
	AppProject: { icon: Shield, className: "text-[var(--resource-argo)]" },
	Releases: { icon: Package, className: "text-muted-foreground" },
	"Namespace Access": { icon: Shield, className: "text-[var(--resource-secret)]" },
	Roles: { icon: KeyRound, className: "text-[var(--resource-secret)]" },
	"Cluster Roles": { icon: Shield, className: "text-[var(--resource-secret)]" },
	Bindings: { icon: Workflow, className: "text-[var(--resource-secret)]" },
	"Service Accounts": { icon: KeyRound, className: "text-[var(--resource-secret)]" },
};

const GROUP_VISUALS: Record<string, ResourceVisual> = {
	"Cluster Overview": { icon: Server, className: "text-[var(--resource-cluster)]" },
	Namespaces: { icon: FolderTree, className: "text-muted-foreground" },
	Workloads: WORKLOAD_VISUAL,
	Network: NETWORK_VISUAL,
	Config: CONFIG_VISUAL,
	Storage: STORAGE_VISUAL,
	Discovered: { icon: Package, className: "text-muted-foreground" },
	"Argo CD": ARGO_VISUAL,
	Helm: { icon: Package, className: "text-muted-foreground" },
	RBAC: { icon: Shield, className: "text-[var(--resource-secret)]" },
	"Managed by Argo app": ARGO_VISUAL,
	"Unmanaged resources": { icon: Package, className: "text-muted-foreground" },
};

export function getResourceKindVisual(kind: string): ResourceVisual {
	return KIND_VISUALS[kind] ?? DEFAULT_VISUAL;
}

export function getResourceGroupVisual(label: string): ResourceVisual {
	if (label.startsWith("Managed by Argo app:")) return GROUP_VISUALS["Managed by Argo app"];
	return GROUP_VISUALS[label] ?? DEFAULT_VISUAL;
}
