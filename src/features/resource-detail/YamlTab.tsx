import { useCallback, useReducer, useRef } from "react";
import type { Diagnostic } from "@codemirror/lint";
import { YamlTabContent } from "@/features/resource-detail/YamlTabContent";
import { YamlTabHeader } from "@/features/resource-detail/YamlTabHeader";
import { formatYamlDocument } from "@/lib/yamlFormat";
import {
	applyYaml,
	getResourceYaml,
	lintKubernetesYaml,
	prepareYamlApply,
	type TauriClient,
} from "../../lib/tauri";
import { useSettingsState } from "../../lib/settings";
import type {
	ResourceSummary,
	YamlEncoding,
	YamlApplyPreview,
	YamlApplyRequest,
	YamlViewMode,
} from "../../lib/types";
import { findYamlFieldRange } from "./yamlTabDiff";

interface YamlTabProps {
	client: TauriClient;
	resource: ResourceSummary;
	yaml: string | undefined;
	yamlLoading: boolean;
	yamlError: boolean;
	yamlErr: unknown;
	yamlViewMode: YamlViewMode;
	onYamlViewModeChange: (mode: YamlViewMode) => void;
	yamlEncoding: YamlEncoding;
	onYamlEncodingChange: (encoding: YamlEncoding) => void;
	onApplied: () => void;
}

type YamlTabState = {
	editing: boolean;
	draftYaml: string;
	draftReady: boolean;
	preview: YamlApplyPreview | null;
	formatError: unknown;
	prepareError: unknown;
	applyError: unknown;
	appliedMessage: string;
	preparing: boolean;
	applying: boolean;
	loadingDraft: boolean;
	showFullDiff: boolean;
};

type YamlTabStateAction = {
	type: "patch";
	payload: Partial<YamlTabState>;
};

const yamlTabReducer = (
	state: YamlTabState,
	action: YamlTabStateAction,
): YamlTabState => {
	if (action.type === "patch") {
		return { ...state, ...action.payload };
	}
	return state;
};

export function YamlTab({
	client,
	resource,
	yaml,
	yamlLoading,
	yamlError,
	yamlErr,
	yamlViewMode,
	onYamlViewModeChange,
	yamlEncoding,
	onYamlEncodingChange,
	onApplied,
}: YamlTabProps) {
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigEnvVar);
	const [state, dispatch] = useReducer(yamlTabReducer, {
		editing: false,
		draftYaml: "",
		draftReady: false,
		preview: null,
		formatError: null,
		prepareError: null,
		applyError: null,
		appliedMessage: "",
		preparing: false,
		applying: false,
		loadingDraft: false,
		showFullDiff: false,
	} satisfies YamlTabState);
	const {
		editing,
		draftYaml,
		draftReady,
		preview,
		formatError,
		prepareError,
		applyError,
		appliedMessage,
		preparing,
		applying,
		loadingDraft,
		showFullDiff,
	} = state;
	const setState = useCallback(
		(payload: Partial<YamlTabState>) => {
			dispatch({ type: "patch", payload });
		},
		[],
	);
	const startApplyRequestId = useRef(0);
	const secretApplyDisabled =
		resource.kind === "Secret" && (resource.apiVersion ?? "v1") === "v1";
	const activeYamlEncoding = yamlEncoding;

	const startApplyFlow = async () => {
		if (loadingDraft) return;
		const requestId = ++startApplyRequestId.current;
		setState({
			appliedMessage: "",
			formatError: null,
			applyError: null,
			prepareError: null,
			preview: null,
			showFullDiff: false,
			draftReady: false,
			loadingDraft: true,
			draftYaml: "",
		});
		try {
			const applyCleanYaml = await getResourceYaml(
				client,
				resource.cluster,
				resource.kind,
				resource.name,
				resource.namespace ?? undefined,
				kubeconfigEnvVar,
				"applyClean",
				yamlEncoding,
			);
			if (requestId !== startApplyRequestId.current) return;
			setState({
				draftYaml: applyCleanYaml,
				draftReady: true,
				editing: true,
			});
		} catch (err) {
			if (requestId !== startApplyRequestId.current) return;
			setState({ draftReady: false, prepareError: err });
		} finally {
			if (requestId !== startApplyRequestId.current) return;
			setState({ loadingDraft: false });
		}
	};

	const cancelApplyFlow = () => {
		setState({
			editing: false,
			draftYaml: "",
			draftReady: false,
			preview: null,
			showFullDiff: false,
			formatError: null,
			prepareError: null,
			applyError: null,
			appliedMessage: "",
		});
	};

	const buildRequest = (): YamlApplyRequest => ({
		clusterContext: resource.cluster,
		kubeconfigEnvVar,
		kind: resource.kind,
		apiVersion: resource.apiVersion,
		group: resource.group,
		version: resource.version,
		plural: resource.plural,
		namespaced: resource.namespaced,
		name: resource.name,
		namespace: resource.namespace,
		yaml: draftYaml,
		yamlEncoding: activeYamlEncoding,
	});

	const buildRequestForYaml = useCallback(
		(value: string): YamlApplyRequest => ({
			clusterContext: resource.cluster,
			kubeconfigEnvVar,
			kind: resource.kind,
			apiVersion: resource.apiVersion,
			group: resource.group,
			version: resource.version,
			plural: resource.plural,
			namespaced: resource.namespaced,
			name: resource.name,
			namespace: resource.namespace,
			yaml: value,
			yamlEncoding: activeYamlEncoding,
		}),
		[activeYamlEncoding, kubeconfigEnvVar, resource],
	);

	const kubernetesDiagnostics = useCallback(
		async (value: string): Promise<Diagnostic[]> => {
			if (!editing || value.trim().length === 0) return [];
			const result = await lintKubernetesYaml(client, buildRequestForYaml(value));
			return result.diagnostics.map((diagnostic) => {
				const range = findYamlFieldRange(value, diagnostic.fieldPath);
				return {
					from: range.from,
					to: range.to,
					severity: diagnostic.severity,
					source: diagnostic.source,
					message: diagnostic.message,
				};
			});
		},
		[buildRequestForYaml, client, editing],
	);

	const prepare = async () => {
		setState({
			preparing: true,
			formatError: null,
			prepareError: null,
			applyError: null,
			preview: null,
			showFullDiff: false,
		});
		try {
			const result = await prepareYamlApply(client, buildRequest());
			setState({ preview: result });
		} catch (err) {
			setState({ prepareError: err });
		} finally {
			setState({ preparing: false });
		}
	};

	const formatDraft = () => {
		setState({
			formatError: null,
			prepareError: null,
			applyError: null,
			preview: null,
			showFullDiff: false,
		});
		try {
			setState({
				draftYaml: formatYamlDocument(draftYaml, activeYamlEncoding),
				appliedMessage: "",
			});
		} catch (err) {
			setState({ formatError: err });
		}
	};

	const apply = async () => {
		if (!preview) return;
		setState({ applying: true, applyError: null });
		try {
			const result = await applyYaml(client, buildRequest());
			setState({
				appliedMessage: `Applied ${result.target.kind}/${result.target.name} with server-side apply.`,
			});
			cancelApplyFlow();
			onApplied();
		} catch (err) {
			setState({ applyError: err });
		} finally {
			setState({ applying: false });
		}
	};

	return (
		<div className="flex min-h-0 flex-col">
			<YamlTabHeader
				editing={editing}
				yamlLoading={yamlLoading}
				yamlError={yamlError}
				secretApplyDisabled={secretApplyDisabled}
				loadingDraft={loadingDraft}
				draftReady={draftReady}
				preview={preview}
				preparing={preparing}
				applying={applying}
				yamlViewMode={yamlViewMode}
				yamlEncoding={activeYamlEncoding}
				onYamlViewModeChange={onYamlViewModeChange}
				onYamlEncodingChange={onYamlEncodingChange}
				onStartApplyFlow={() => void startApplyFlow()}
				onFormat={formatDraft}
				onPrepare={prepare}
				onApply={apply}
				onCancel={cancelApplyFlow}
			/>

			<YamlTabContent
				secretApplyDisabled={secretApplyDisabled}
				appliedMessage={appliedMessage}
				yamlLoading={yamlLoading}
				yamlError={yamlError}
				yamlErr={yamlErr}
				editing={editing}
				yaml={yaml}
				resource={resource}
				draftYaml={draftYaml}
				draftReady={draftReady}
				loadingDraft={loadingDraft}
				preview={preview}
				formatError={formatError}
				prepareError={prepareError}
				applyError={applyError}
				preparing={preparing}
				applying={applying}
				showFullDiff={showFullDiff}
				activeYamlEncoding={activeYamlEncoding}
				onEditChange={(value) => {
					setState({
						draftYaml: value,
						preview: null,
						showFullDiff: false,
						formatError: null,
						prepareError: null,
						applyError: null,
						appliedMessage: "",
					});
				}}
				onEditFormat={formatDraft}
				onEditPrepare={prepare}
				onEditApply={apply}
				onEditCancel={cancelApplyFlow}
				onHideEditMessage={() => {
					setState({ appliedMessage: "" });
				}}
				onToggleEditFullDiff={() => {
					setState({ showFullDiff: !showFullDiff });
				}}
				extraDiagnostics={kubernetesDiagnostics}
			/>
		</div>
	);
}
