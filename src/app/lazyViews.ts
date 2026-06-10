import { lazy } from "react";

export const ResourceList = lazy(() =>
	import("../features/resources/ResourceList").then((module) => ({
		default: module.ResourceList,
	})),
);

export const ResourceDetailPanel = lazy(() =>
	import("../features/resource-detail/ResourceDetailPanel").then((module) => ({
		default: module.ResourceDetailPanel,
	})),
);

export const GitOpsPanel = lazy(() =>
	import("../features/gitops/GitOpsPanel").then((module) => ({
		default: module.GitOpsPanel,
	})),
);

export const ArgoDetailPanel = lazy(() =>
	import("../features/argo/ArgoDetailPanel").then((module) => ({
		default: module.ArgoDetailPanel,
	})),
);

export const FluxDetailPanel = lazy(() =>
	import("../features/gitops/FluxDetailPanel").then((module) => ({
		default: module.FluxDetailPanel,
	})),
);

export const HelmPanel = lazy(() =>
	import("../features/helm").then((module) => ({
		default: module.HelmPanel,
	})),
);

export const HelmDetailPanel = lazy(() =>
	import("../features/helm").then((module) => ({
		default: module.HelmDetailPanel,
	})),
);

export const RbacPanel = lazy(() =>
	import("../features/rbac").then((module) => ({
		default: module.RbacPanel,
	})),
);

export const IncidentCockpit = lazy(() =>
	import("../features/incidents").then((module) => ({
		default: module.IncidentCockpit,
	})),
);

export const SettingsPage = lazy(() =>
	import("../features/settings/SettingsPage").then((module) => ({
		default: module.SettingsPage,
	})),
);

export const WorkspaceLauncher = lazy(() =>
	import("../features/workspaces").then((module) => ({
		default: module.WorkspaceLauncher,
	})),
);

export const WorkspaceOverview = lazy(() =>
	import("../features/workspaces").then((module) => ({
		default: module.WorkspaceOverview,
	})),
);

export const WorkspacePortForwardsPage = lazy(() =>
	import("../features/live-sessions").then((module) => ({
		default: module.WorkspacePortForwardsPage,
	})),
);
