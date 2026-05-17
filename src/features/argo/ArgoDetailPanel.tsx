import { useState } from "react";
import { X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type {
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	ArgoAppProjectSummary,
} from "@/lib/types";
import {
	ArgoApplicationDetail,
	ArgoApplicationYaml,
} from "./ArgoApplicationDetail";
import {
	ArgoApplicationSetDetail,
	ArgoApplicationSetYaml,
} from "./ArgoApplicationSetDetail";
import {
	ArgoAppProjectDetail,
	ArgoAppProjectYaml,
} from "./ArgoAppProjectDetail";

type Tab = "details" | "yaml";
type ArgoDetailItem =
	| ArgoApplicationSummary
	| ArgoApplicationSetSummary
	| ArgoAppProjectSummary;

const PANEL_CLASS =
	"flex h-full min-w-0 flex-col overflow-hidden border-l bg-card";
const PANEL_HEADER_CLASS =
	"flex shrink-0 items-center justify-between border-b px-4 py-3";
const PANEL_TITLE_CLASS = "truncate whitespace-nowrap text-sm font-semibold";
const PANEL_TABS_CLASS = "flex shrink-0 border-b";
const PANEL_TAB_CLASS =
	"rounded-none border-b-2 border-transparent bg-transparent px-4 py-2 text-[13px] text-muted-foreground shadow-none transition-colors hover:text-foreground data-[state=active]:border-primary data-[state=active]:bg-transparent data-[state=active]:text-primary data-[state=active]:shadow-none";
const PANEL_BODY_CLASS = "flex-1 overflow-y-auto p-4";

function ArgoDetailContent({
	app,
	isApp,
	isAppSet,
	isAppProject,
}: {
	app: ArgoDetailItem;
	isApp: boolean;
	isAppSet: boolean;
	isAppProject: boolean;
}) {
	return (
		<>
			{isApp && <ArgoApplicationDetail app={app as ArgoApplicationSummary} />}
			{isAppSet && (
				<ArgoApplicationSetDetail appset={app as ArgoApplicationSetSummary} />
			)}
			{isAppProject && (
				<ArgoAppProjectDetail project={app as ArgoAppProjectSummary} />
			)}
		</>
	);
}

function ArgoYamlContent({
	app,
	isApp,
	isAppSet,
	isAppProject,
}: {
	app: ArgoDetailItem;
	isApp: boolean;
	isAppSet: boolean;
	isAppProject: boolean;
}) {
	return (
		<>
			{isApp && <ArgoApplicationYaml app={app as ArgoApplicationSummary} />}
			{isAppSet && (
				<ArgoApplicationSetYaml appset={app as ArgoApplicationSetSummary} />
			)}
			{isAppProject && (
				<ArgoAppProjectYaml project={app as ArgoAppProjectSummary} />
			)}
		</>
	);
}

export function ArgoDetailPanel({
	app,
	onClose,
}: {
	app: ArgoDetailItem;
	onClose: () => void;
}) {
	const [activeTab, setActiveTab] = useState<Tab>("details");
	const isAppProject = "description" in app;
	const isAppSet = !isAppProject && "status" in app;
	const isApp = !isAppProject && !isAppSet;
	const title = app.name + (app.namespace ? ` (${app.namespace})` : "");

	return (
		<div className={PANEL_CLASS}>
			<div className={PANEL_HEADER_CLASS}>
				<span className={PANEL_TITLE_CLASS}>{title}</span>
				<Button
					type="button"
					variant="ghost"
					size="icon"
					className="size-7 text-muted-foreground"
					onClick={onClose}
					aria-label="Close panel"
				>
					<X className="size-4" />
				</Button>
			</div>
			<Tabs
				value={activeTab}
				onValueChange={(value) => setActiveTab(value as Tab)}
				className="min-h-0 flex-1 gap-0"
			>
				<div className={PANEL_TABS_CLASS}>
					<TabsList className="h-auto rounded-none bg-transparent p-0">
						<TabsTrigger className={PANEL_TAB_CLASS} value="details">
							Details
						</TabsTrigger>
						<TabsTrigger className={PANEL_TAB_CLASS} value="yaml">
							YAML
						</TabsTrigger>
					</TabsList>
				</div>
				<div className={PANEL_BODY_CLASS}>
					<TabsContent value="details" className="m-0">
						<ArgoDetailContent
							app={app}
							isApp={isApp}
							isAppSet={isAppSet}
							isAppProject={isAppProject}
						/>
					</TabsContent>
					<TabsContent value="yaml" className="m-0">
						<ArgoYamlContent
							app={app}
							isApp={isApp}
							isAppSet={isAppSet}
							isAppProject={isAppProject}
						/>
					</TabsContent>
				</div>
			</Tabs>
		</div>
	);
}
