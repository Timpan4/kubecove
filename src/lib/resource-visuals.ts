import {
	Box,
	Boxes,
	BriefcaseBusiness,
	Cable,
	CircleAlert,
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
	surfaceClassName: string;
	badgeClassName: string;
}

type ResourceTone =
	| "argo"
	| "cluster"
	| "config"
	| "default"
	| "deployment"
	| "network"
	| "pod"
	| "replicaset"
	| "secret"
	| "storage"
	| "workload";

const RESOURCE_TONE_CLASSES: Record<
	ResourceTone,
	Omit<ResourceVisual, "icon">
> = {
	argo: {
		className: "text-[var(--resource-argo)]",
		surfaceClassName: "resource-tone-argo-surface",
		badgeClassName: "resource-tone-argo-badge",
	},
	cluster: {
		className: "text-[var(--resource-cluster)]",
		surfaceClassName: "resource-tone-cluster-surface",
		badgeClassName: "resource-tone-cluster-badge",
	},
	config: {
		className: "text-[var(--resource-config)]",
		surfaceClassName: "resource-tone-config-surface",
		badgeClassName: "resource-tone-config-badge",
	},
	default: {
		className: "text-muted-foreground",
		surfaceClassName: "resource-tone-default-surface",
		badgeClassName: "resource-tone-default-badge",
	},
	deployment: {
		className: "text-[var(--resource-deployment)]",
		surfaceClassName: "resource-tone-deployment-surface",
		badgeClassName: "resource-tone-deployment-badge",
	},
	network: {
		className: "text-[var(--resource-network)]",
		surfaceClassName: "resource-tone-network-surface",
		badgeClassName: "resource-tone-network-badge",
	},
	pod: {
		className: "text-[var(--resource-pod)]",
		surfaceClassName: "resource-tone-pod-surface",
		badgeClassName: "resource-tone-pod-badge",
	},
	replicaset: {
		className: "text-[var(--resource-replicaset)]",
		surfaceClassName: "resource-tone-replicaset-surface",
		badgeClassName: "resource-tone-replicaset-badge",
	},
	secret: {
		className: "text-[var(--resource-secret)]",
		surfaceClassName: "resource-tone-secret-surface",
		badgeClassName: "resource-tone-secret-badge",
	},
	storage: {
		className: "text-[var(--resource-storage)]",
		surfaceClassName: "resource-tone-storage-surface",
		badgeClassName: "resource-tone-storage-badge",
	},
	workload: {
		className: "text-[var(--resource-workload)]",
		surfaceClassName: "resource-tone-workload-surface",
		badgeClassName: "resource-tone-workload-badge",
	},
};

function resourceVisual(icon: LucideIcon, tone: ResourceTone): ResourceVisual {
	return { icon, ...RESOURCE_TONE_CLASSES[tone] };
}

const WORKLOAD_VISUAL = resourceVisual(Boxes, "workload");
const NETWORK_VISUAL = resourceVisual(Network, "network");
const CONFIG_VISUAL = resourceVisual(FileCog, "config");
const STORAGE_VISUAL = resourceVisual(HardDrive, "storage");
const ARGO_VISUAL = resourceVisual(GitBranch, "argo");
const DEFAULT_VISUAL = resourceVisual(Package, "default");

const KIND_VISUALS: Record<string, ResourceVisual> = {
	Pod: resourceVisual(Box, "pod"),
	Deployment: resourceVisual(Rocket, "deployment"),
	ReplicaSet: resourceVisual(Box, "replicaset"),
	StatefulSet: resourceVisual(Database, "storage"),
	DaemonSet: resourceVisual(Server, "workload"),
	Job: resourceVisual(BriefcaseBusiness, "cluster"),
	CronJob: resourceVisual(Clock3, "cluster"),
	Service: resourceVisual(Network, "network"),
	Ingress: resourceVisual(Globe2, "network"),
	ConfigMap: resourceVisual(FileCog, "config"),
	Secret: resourceVisual(KeyRound, "secret"),
	PersistentVolumeClaim: resourceVisual(HardDrive, "storage"),
	PersistentVolume: resourceVisual(Database, "storage"),
	StorageClass: resourceVisual(Layers, "storage"),
	Node: resourceVisual(Server, "cluster"),
	Applications: resourceVisual(GitBranch, "argo"),
	Application: resourceVisual(GitBranch, "argo"),
	ApplicationSets: resourceVisual(Workflow, "argo"),
	ApplicationSet: resourceVisual(Workflow, "argo"),
	AppProjects: resourceVisual(Shield, "argo"),
	AppProject: resourceVisual(Shield, "argo"),
	Releases: resourceVisual(Package, "default"),
	"Namespace Access": resourceVisual(Shield, "secret"),
	Roles: resourceVisual(KeyRound, "secret"),
	"Cluster Roles": resourceVisual(Shield, "secret"),
	Bindings: resourceVisual(Workflow, "secret"),
	"Service Accounts": resourceVisual(KeyRound, "secret"),
};

const GROUP_VISUALS: Record<string, ResourceVisual> = {
	"Cluster Overview": resourceVisual(Server, "cluster"),
	Namespaces: resourceVisual(FolderTree, "default"),
	Workloads: WORKLOAD_VISUAL,
	Network: NETWORK_VISUAL,
	Config: CONFIG_VISUAL,
	Storage: STORAGE_VISUAL,
	Discovered: DEFAULT_VISUAL,
	"Argo CD": ARGO_VISUAL,
	Helm: DEFAULT_VISUAL,
	Incidents: resourceVisual(CircleAlert, "cluster"),
	RBAC: resourceVisual(Shield, "secret"),
	"Port Forwards": resourceVisual(Cable, "network"),
	"Managed by Argo app": ARGO_VISUAL,
	"Unmanaged resources": DEFAULT_VISUAL,
};

export function getResourceKindVisual(kind: string): ResourceVisual {
	return KIND_VISUALS[kind] ?? DEFAULT_VISUAL;
}

export function getResourceGroupVisual(label: string): ResourceVisual {
	if (label.startsWith("Managed by Argo app:")) return GROUP_VISUALS["Managed by Argo app"];
	return GROUP_VISUALS[label] ?? DEFAULT_VISUAL;
}
