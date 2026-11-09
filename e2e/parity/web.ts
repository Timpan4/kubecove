import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { chromium, type Locator, type Page } from "playwright-core";
import { chromeExecutable, freePort, readinessTimeoutMs, startIdentity, stopGroup, waitFor } from "../../.agents/skills/verify-kubecove/scripts/processes";
import type { Contract, Step } from "./contract";
import { adapterAction } from "./web-adapters";
import { argoAction } from "./web-argo";
import { coverageAction } from "./web-coverage";
import { checkCoverage } from "./web-coverage-checks";
import { startCodeCoverage } from "./web-coverage-map";
import { detailStatusAction } from "./web-detail-status";
import { detailsAction } from "./web-details";
import { diagnosticsAction } from "./web-diagnostics";
import { topologySpikeAction } from "./web-topology-spike";
import { feedbackAction } from "./web-feedback";
import { incidentDiagnosticsAction } from "./web-incident-diagnostics";
import { kubeconfigSettingsAction } from "./web-kubeconfig-settings";
import { liveSessionAction } from "./web-live-sessions";
import { logRowAndRestoreAction } from "./web-logs-restore";
import { expectLogToolbarFit } from "./web-logs-toolbar";
import { expectDetailScroll, expectResourceEvent } from "./web-detail-scroll";
import { mcpAction } from "./web-mcp";
import { remoteAction } from "./web-remote";
import { navigationAction } from "./web-navigation";
import { panelAction } from "./web-panels";
import { multiContextAction } from "./web-multi-context";
import { resourceFilterAction } from "./web-resource-filters";
import { resourceRow, resourceTableAction } from "./web-resource-table";
import { sessionHealthAction } from "./web-session-health";
import { seedsWorkspace, seedWorkspace, shellAction, storeReloadHolds } from "./web-shell";
import { sidebarLiveAction } from "./web-sidebar-live";
import { terminalAction } from "./web-terminal";
import { timingAction } from "./web-timing";
import { topologyAction } from "./web-topology";
import { expectRetainedWorkspaceFields, workspaceAction } from "./web-workspaces";
import { yamlEditorAction } from "./web-yaml-editor";
import { yamlPermissionAction } from "./web-yaml-permissions";

export async function action(page: Page, step: Step) {
	const search = page.getByRole("textbox", { name: "Search resources" });
	const close = page.getByRole("button", { name: "Close resource details" });
	switch (step.action) {
        case "expectOverviewShortcuts": case "overviewShortcut": await shellAction(page, step); break;
        case "expectOverviewScope": case "expectCompareTotal": await shellAction(page, step); break;
        case "retryOverview": case "expectOverviewRetry": await shellAction(page, step); break;
        case "dragKind": await shellAction(page, step); break;
        case "expectEmptyLauncher": case "expectEntryCount": await shellAction(page, step); break;
        case "startWithoutWorkspace": case "expectNoWorkspaceShell": case "expectPaletteUnavailable": await shellAction(page, step); break;
        case "copyErrorDetail": await shellAction(page, step); break;
        case "expectForegroundLoading": case "expectFriendlyError": case "inspectRequiredPermission": case "expectPermissionTarget": case "returnToSource": case "copyResourceName": case "expectCopyToast": case "dismissCopyToast": await feedbackAction(page, step); break;
        case "toggleResourcePin": case "expectWorkspaceEntry": case "openWorkspaceEntry": await shellAction(page, step); break;
        case "reorderKindKey": case "resetKindOrder": case "expectRemoteHost": case "expectRemoteHostTooltip": case "expectNoRemoteHost": case "dropRemoteConnection": await shellAction(page, step); break;
        case "remoteSessionSignal": case "expectRemoteNotice": case "expectNoRemoteNotice": case "clickRemoteNoticeButton": await remoteAction(page, step); break;
        case "configureShellRead": case "retryLauncher": case "expectLauncherRetry": await shellAction(page, step); break;
        case "expectActivePage": case "expectGitOpsSelection": await shellAction(page, step); break;
        case "overviewOperation": case "overviewIncident": case "reviewIncidentAction": case "expectDetailTab": await shellAction(page, step); break;
        case "expectTopologyDenied": case "mockTiming": case "expectWatchStart": await timingAction(page, step); break;
        case "coverageSelect": await coverageAction(page, step); break;
        case "coverageCopy": await coverageAction(page, step); break;
        case "coverageScript": await coverageAction(page, step); break;
        case "coverageCheck": await checkCoverage(page, step.check); break;
        case "coverageClick": case "coverageFill": case "coverageText": case "paletteKey": case "expectPaletteOpen": case "deleteSavedWorkspace": case "importDecision": case "cancelImport": case "searchEvents": await coverageAction(page, step); break;
        case "panelControl": case "expectPanelControl": case "skipGraph": case "mapKey": case "expectMapFocus": case "expectNoMapFocus": case "expectMapNodeInView": case "panMap": case "clickMapControl": case "expectMapControlFocus": case "expectMapZoom": case "resizeDetail": case "expectDetailResize": await panelAction(page, step); break;
        case "terminalFit": case "terminalOutput": case "terminalSelect": case "terminalCopy": case "terminalPaste": case "terminalScroll": case "terminalPrimaryPaste": case "terminalDragOutside": case "terminalRelease": case "terminalFocus": case "terminalRightClick": case "expectTerminalCursor":
            await terminalAction(page, step); break;
        case "failWorkspaceContext": case "expectWorkspaceReadWarning": case "expectContextUnavailable":
            await workspaceAction(page, step); break;
        case "editSavedWorkspace": case "expectNoSavedWorkspace": case "saveWorkspace": case "cancelWorkspaceEdit": case "toggleWorkspaceContext": case "expectOverviewHealth": case "expectDetailContext": case "exportSavedWorkspace": case "expectExportedShortcut":
            await workspaceAction(page, step); break;
        case "sidebarToggle": case "sidebarSelect": case "expectSidebarRow": case "collapseSidebar": case "expectSidebarBranch": case "scrollSidebar": case "expectSidebarReveal": case "refreshPage": await sidebarLiveAction(page, step); break;
        case "openNamespaceList": case "expectNamespaceRow": case "resizeViewport": case "openNavigationDrawer": case "expectNavigationDrawer": case "sidebarKey": await navigationAction(page, step); break;
        case "replayPageReads": case "focusStalePage": case "expectPageText": case "searchCustomKinds": case "searchKindPicker": case "expectCustomGroup": case "toggleKind": case "openKindPicker": case "closeKindPicker": case "selectAllKinds": case "expectKindChecked": await sidebarLiveAction(page, step); break;
        case "openNamespacePicker": case "closeNamespacePicker": case "searchNamespacePicker": case "selectAllNamespaces": case "toggleNamespace": case "expectNamespaceChecked": case "expectNamespaceTrigger": case "selectHealthFilter": case "expectHealthFilter": case "chooseGitOpsOwner": case "clearResourceFilters": case "setHistoricalReplicaSets": await resourceFilterAction(page, step); break;
        case "toggleDetailDisclosure": case "expectNoPageText": await detailStatusAction(page, step); break;
        case "fillMcpPort": case "mcpControl": case "expectMcpState": case "expectMcpCopy": case "expectMcpRequests":
            await mcpAction(page, step); break;
        case "configureArgoMock": case "argoControl": case "focusArgoResource": case "expectNoSavedArgoProfile": case "copyArgoValue": case "configureArgoInventory": case "expandArgoDiff": case "reconnectArgoProfile":
        case "connectArgoProfile": case "disconnectArgoProfile": case "reviewArgoOperation": case "confirmArgoReview": case "cancelArgoReview": case "expectArgoReview": case "expectArgoText": case "argoDocument":
        case "expectArgoPopover": case "dismissArgoPopover": case "expectArgoCheckbox": case "toggleArgoOption": case "expectNoArgoControl":
        case "selectArgoPreference": case "expectSavedArgoProfile": case "configureArgoSync": case "selectArgoHistory": case "expectArgoWorkspaceTransport": case "completeArgoOperation": case "confirmArgoTunnel":
        case "expectArgoFocusInPopover": case "argoConnectionDialog": case "tabArgoPopover": case "expectArgoTriggerFocus": case "expectArgoOptionFocusable": case "expectArgoOperationState": case "controlClock": case "advanceClock": case "selectArgoConnection": case "fillArgoConnectionField": case "expectArgoConnectionField":
            await argoAction(page, step); break;
        case "expectYamlDiagnostic": case "expectNoYamlDiagnostic": case "expectYamlApplyDescription": case "expectNoYamlApplyDescription": case "expectActionLoadError": case "expectNoActionLoadError": case "retryActionPermission": case "expectActionLock":
            await yamlPermissionAction(page, step); break;
        case "cancelYamlEdit": case "revealSecretValue": case "hideSecretValue": case "expectSecretValue": case "expectYamlLens": case "refreshLatencyReport": case "clearLatencyReport": case "expectLatencyEmpty": case "expectLatencyTiming": case "openDocumentation": case "expectDocumentationRequest":
            await adapterAction(page, step); break;
        case "openIncidentResources": case "updateStatusControl": case "expectUpdateControl": case "copyLatencyReport": case "expectLatencyReportCopy":
            await incidentDiagnosticsAction(page, step); break;
        case "expectViewTiming": case "copyInteractions": case "clearInteractions": case "expectInteractionCopy": case "typeSettingsField":
            await diagnosticsAction(page, step); break;
        case "runTopologySpike": case "expectTopologySpikeText": await topologySpikeAction(page, step); break;
        case "expectWorkspaceExport": {
            await loadSettingsMocks(page);
            const content = await (await page.waitForFunction(() => window.paritySettingsMocks.workspaceExportMock())).jsonValue();
            const document = JSON.parse(content);
            const items = document.kind === "WorkspaceList" ? document.items : [document];
            if (!items.some((item: { spec: { displayName: string; scope: { clusterContext: string; namespaces: string[] } } }) => item.spec.displayName === step.name && item.spec.scope.clusterContext === "mock-dev" && item.spec.scope.namespaces.includes("payments"))) throw new Error("Export ignored the imported workspace scope");
            expectRetainedWorkspaceFields(items, step.name);
            break;
        }
        case "fillSettingsField": await page.getByLabel(step.field, { exact: true }).fill(step.value); break;
        case "clickSettingsButton": await page.getByRole("button", { name: step.label, exact: true }).click(); break;
        case "expectSettingsText": await page.getByText(step.value, { exact: false }).first().waitFor(); break;
        case "expectNoSettingsText":
            // Waits for removal to render; text that never rendered passes on the first poll.
            await page.waitForFunction((value) => !document.body.textContent?.includes(value), step.value);
            break;
        case "expectMaskedToken":
            if (await page.getByLabel("Server token", { exact: true }).getAttribute("type") !== "password") throw new Error("Credential is not masked");
            break;
        case "expectRuntimeEndpoint": {
            await loadSettingsMocks(page);
            await page.waitForFunction((expected) => (window.paritySettingsMocks.remoteMockStatus()?.serverUrl ?? "local") === expected, step.value);
            break;
        }
        case "reopenStoredSettings":
            {
                const directory = await mkdtemp(join(process.env.TMPDIR ?? join(process.cwd(), ".t"), "parity-settings-"));
                try {
                    const snapshot = await page.evaluate("import('/src/lib/tauri-dev-mocks.ts').then(m => m.createDevMockTauriClient().invoke('get_kubeconfig_sources'))");
                    const path = join(directory, "sources.json");
                    await writeFile(path, JSON.stringify(snapshot));
                    await page.getByRole("button", { name: "Back to app", exact: true }).click();
                    await page.reload();
                    await page.evaluate(`import('/src/lib/tauri-dev-mocks.ts').then(m => m.restoreMockKubeconfigSources(${await readFile(path, "utf8")}))`);
                    await page.getByRole("button", { name: "Open settings", exact: true }).click();
                } finally { await rm(directory, { recursive: true }); }
            }
            break;
        case "configureSettingsMock":
            await page.evaluate(`import('/src/lib/settings-integration-mocks.ts').then(m => m.configureSettingsMock(${JSON.stringify(step.script)}))`);
            break;
        case "attemptUnverifiedUpdate":
            await page.getByRole("button", { name: "Install", exact: true }).click();
            await page.getByText("Invalid update signature", { exact: true }).waitFor();
            break;
        case "configureForward": case "forwardControl": case "expectForwardState": case "expectForwardNotice":case "execControl": case "expectExecState": case "replaySessions": case "confirmExecLeave": case "configurePreset": case "presetControl": await liveSessionAction(page, step); break;
        case "openLiveSessionPopover": case "expectLiveSessionRow": case "liveSessionControl": case "expectHealthTooltip": case "expectTimestampTooltip": case "expectEventTimeTooltip": await sessionHealthAction(page, step); break;
        case "expectLiveState":
            await page.getByRole("button", { name: "Live updates and refresh options" }).first().getByText(step.label, { exact: true }).waitFor();
            if (step.notice) await page.getByText(step.notice, { exact: true }).first().waitFor();
            break;
        case "replayLiveData": await page.evaluate(`import('/src/lib/tauri-dev-mock-live.ts').then(m => m.replayLiveData(${JSON.stringify(step.script)}))`); break;
        case "sortResourceTable": case "pressResourceRow": case "expectResourceOrder": case "toggleResourceGroup": case "expectResourceGroup": case "openResourceRow": case "expectResourceCell": case "expectResourceCellFits": case "expectResourceColumns": case "expectResourceActivity": await resourceTableAction(page, step); break;
        case "expectRowContexts": case "filterContexts": case "expectResourceSummary": case "selectMapContext": case "expectMapContext": await multiContextAction(page, step); break;
        case "expectResourceStatus": await resourceRow(page, step).getByText(step.status, { exact: true }).waitFor(); break;
        case "openCustomKind": {
            if (step.surface === "sidebar") {
                await page.getByRole("button", { name: "Expand Custom Resources", exact: true }).click();
                await page.getByRole("button", { name: "Expand kustomize.toolkit.fluxcd.io", exact: true }).click();
                await page.getByRole("treeitem").filter({ has: page.getByText(step.kind, { exact: true }) }).last().click();
            } else {
                await page.getByRole("button", { name: "Edit resource kinds" }).click();
                await page.getByRole("textbox", { name: "Search resource kinds" }).fill(step.kind);
                const checkbox = page.locator("label").filter({ hasText: step.kind }).last().getByRole("checkbox");
                if (await checkbox.getAttribute("aria-checked") !== "true") await checkbox.click();
                await page.keyboard.press("Escape");
            }
            break;
        }
        case "expectUsageFooter": case "openUsageTree": case "expectUsageProcess": case "expectNoUsageFooter": await topologyAction(page, step); break;
        case "selectNamespace": await page.getByRole("treeitem").filter({ has: page.getByText(step.namespace, { exact: true }) }).first().click(); break;
        case "setTopologyMode": case "expectTopology": case "expectTopologyEdge": case "selectTopologyNode": case "expectTopologyEmpty": case "expectNoTopologyNode": case "expectResourceMetrics": await topologyAction(page, step); break;
        case "expectYamlAbsent": case "clickActionEntry": case "fillContainerImage": case "openActionEditor": case "fillActionField": case "chooseActionRevision": await yamlEditorAction(page, step); break;
        case "selectYamlShape": case "selectYamlEncoding": case "expectYamlPicker": case "replaceYamlDraft": case "formatYaml": case "expectYamlDocument": case "foldYamlLine": case "expectYamlVisibleLines": await yamlEditorAction(page, step); break;
		case "clickDetailControl": await detailsAction(page, step); break;
		case "openResources":
			await action(page, { action: "openPage", page: "Workspace Overview" });
			await page.getByRole("button", { name: "Resources", exact: true }).first().click();
			break;
		case "search": await search.fill(step.value); break;
		case "expectSearch": if (await search.inputValue() !== step.value) throw new Error("Search changed after refresh"); break;
		case "expectResource": case "openResource": {
			const row = resourceRow(page, step);
			await row.waitFor({ state: "visible" });
			if (await row.count() !== 1) throw new Error("Resource row is ambiguous");
			if (step.action === "openResource") await row.getByRole("button", { name: `Open resource ${step.name}`, exact: true }).click();
			break;
		}
		case "expectDetail": {
			const header = close.locator("xpath=ancestor::header");
			await header.getByRole("heading", { name: step.name, exact: true }).waitFor();
			await header.getByText(`${step.kind} / ${step.namespace}`, { exact: true }).waitFor();
			break;
		}
		case "showYaml": await page.getByRole("tab", { name: "YAML", exact: true }).click(); break;
		case "expectYaml": for (const fragment of step.contains) await page.getByText(fragment, { exact: false }).first().waitFor(); break;
		case "closeDetail": await close.click(); break;
		case "expectDetailClosed": await close.waitFor({ state: "detached" }); break;
		case "expectEmpty":
			await page.getByText("No resources match current scope and filters.", { exact: true }).waitFor();
			if (await page.locator('tr:has(button[aria-label^="Open resource "])').count()) throw new Error("Rows remain in empty search");
			break;
		case "refresh":
			await page.getByRole("button", { name: "Live updates and refresh options" }).first().click();
			await page.getByRole("button", { name: "Refresh (clear cache)" }).click();
			await page.getByRole("button", { name: "Refresh (clear cache)" }).waitFor({ state: "visible" });
			await page.waitForFunction(() => {
				const button = [...document.querySelectorAll("button")].find((item) => item.textContent?.trim() === "Refresh (clear cache)");
				return button instanceof HTMLButtonElement && !button.disabled;
			});
			if (await page.getByText("Refresh failed:", { exact: false }).count()) throw new Error("Refresh failed");
			await page.keyboard.press("Escape"); break;
		case "expectClusterResource": case "expectNoClusterResource": {
			const row = page.locator("tr").filter({ has: page.getByRole("button", { name: `Open resource ${step.name}`, exact: true }) }).filter({ has: page.getByText(step.kind, { exact: true }) });
			// Absence is checked after a preceding step waited for the new scope's rows.
			if (step.action === "expectNoClusterResource") { if (await row.count()) throw new Error(`${step.kind} ${step.name} is still listed`); }
			else await row.first().waitFor({ state: "visible" });
			break;
		}
		case "openPage": {
			// Rows with children also name their expand toggle.
			const label = literal(step.page);
			await page.getByRole("treeitem", { name: new RegExp(`^(?:(?:Expand|Collapse) ${label} )?${label}$`) }).click();
			break;
		}
		case "showTab": await page.getByRole("tab", { name: step.tab, exact: true }).click(); break;
		case "filterEvents": await page.getByRole("radiogroup", { name: "Filter events by type" }).getByRole("radio", { name: step.filter, exact: true }).click(); break;
        case "expectDetailScroll": await expectDetailScroll(page, step.view); break;
		case "expectEventCount":
			if (step.count) await eventRows(page).nth(step.count - 1).waitFor();
			await eventRows(page).nth(step.count).waitFor({ state: "detached" });
			break;
		case "expectEvent": await expectResourceEvent(page, step); break;
		case "expectLiveSessionCount": await page.getByRole("button", { name: `Open live sessions (${step.count} active)`, exact: true }).waitFor(); break;
		case "filterIncidents": await page.locator('[aria-label="Filter incident signals"]').getByRole("button", { name: new RegExp(`^${literal(step.filter)} \\d+$`) }).click(); break;
		case "expectIncidentGuide": await page.getByRole("heading", { level: 3, name: step.title, exact: true }).waitFor(); break;
		case "expectGitOpsCount":
			await page.locator("div.justify-between").filter({ has: page.getByText(step.label, { exact: true }) }).locator("strong").filter({ hasText: new RegExp(`^${step.count}$`) }).waitFor();
			break;
		case "expectPortForward": await page.getByRole("cell").getByText(step.title, { exact: true }).waitFor(); break;
		case "expectLogLines":
			// Headlines of the rendered rows, in display order.
			await page.waitForFunction((lines) => JSON.stringify([...document.querySelectorAll("[data-log-row]")].map((row) => row.querySelector('span[class*="text-[12.5px]"]')?.textContent?.trim())) === JSON.stringify(lines), step.lines);
			break;
		case "expectLogToolbarFit": await expectLogToolbarFit(page, step.state); break;
		case "filterLogs": await page.getByRole("textbox", { name: "Filter log lines" }).fill(step.value); break;
		case "toggleLogLevel": await page.getByRole("group", { name: "Filter by level" }).getByRole("button", { name: new RegExp(`^${step.level} `) }).click(); break;
		case "toggleLogRow": case "toggleLogExpandAll": case "expectLogRowOpen": case "copyLogLine": case "expectLogTimestampTooltip": case "expectRestorePrompt": case "restorePromptControl": await logRowAndRestoreAction(page, step); break;
		case "expectLogsEmpty": await page.getByText(step.title, { exact: true }).waitFor(); break;
		case "selectRevision": await revisionButton(page, step.label).click(); break;
		case "expectRevisionDiff": {
			const header = page.locator("div.border-b").filter({ hasText: "compared with" });
			await header.locator("span.font-medium", { hasText: new RegExp(`^${literal(step.selected)}$`) }).waitFor();
			await header.getByRole("combobox", { name: "Comparison revision" }).filter({ hasText: new RegExp(`^\\s*${literal(step.comparison)}\\s*$`) }).waitFor();
			await containsLines(page.getByRole("tabpanel").locator("pre"), step.contains);
			break;
		}
		case "reviewAction": await page.getByRole("button", { name: step.label, exact: true }).click(); break;
		case "editReplicas":
			await page.locator('[data-resource-action="replicas"]').getByRole("button", { name: "Edit", exact: true }).click();
			await page.getByRole("spinbutton", { name: "Desired replicas" }).fill(step.value);
			break;
		case "expectReview": {
			const dialog = page.getByRole("dialog");
			await dialog.getByRole("heading", { name: step.title, exact: true }).waitFor();
			await dialog.getByText(step.description, { exact: true }).waitFor();
			await containsLines(dialog.getByLabel("Server result diff"), step.contains);
			break;
		}
		case "confirmDeletion": await page.getByRole("dialog").getByRole("textbox", { name: "Type resource name to confirm" }).fill(step.value); break;
		// Forced, so a disabled control is clicked like a user would and must do nothing.
		case "executeReview": await page.getByRole("dialog").getByRole("button", { name: /^(Apply|Delete)$/ }).click({ force: true }); break;
		case "closeReview": await page.getByRole("dialog").getByRole("button", { name: "Cancel", exact: true }).click(); break;
		case "expectNoReview": await page.getByRole("dialog").waitFor({ state: "detached" }); break;
		case "expectActionResult": await page.getByRole("status").filter({ hasText: new RegExp(`^${literal(step.text)}$`) }).waitFor(); break;
		case "expectNoActionResult": if (await page.locator('p[role="status"].text-emerald-300').count()) throw new Error("An action result is shown"); break;
		case "editYaml": await page.getByRole("button", { name: "Edit YAML", exact: true }).click({ force: true }); break;
		case "applyYaml": await page.getByRole("button", { name: "Apply", exact: true }).click({ force: true }); break;
		case "dryRunYaml": await page.getByRole("button", { name: "Dry run", exact: true }).click(); break;
		case "expectDryRun": await containsLines(page.locator("div.font-mono.text-xs.leading-relaxed"), step.contains); break;
		case "expectYamlNotice": await page.getByText(step.title, { exact: true }).first().waitFor(); break;
		case "expectNoYamlNotice": if (await page.getByText(step.title, { exact: true }).count()) throw new Error(`${step.title} is shown`); break;
		case "expectYamlReadOnly":
			await page.getByRole("button", { name: "Edit YAML", exact: true }).waitFor();
			if (await page.getByRole("button", { name: "Dry run", exact: true }).count()) throw new Error("The YAML draft editor opened");
			break;
		// Absence is checked after a preceding step waited for the new scope's rows.
		case "expectNoResource": await resourceRow(page, step).waitFor({ state: "detached" }); break;
		case "openLauncher": await shellAction(page, step); break;
		case "openSavedWorkspace": await page.getByRole("button", { name: `Open ${step.name}`, exact: true }).click(); await page.waitForURL(/#\/workspace\/[^/]+\//); break;
		case "expectSavedWorkspace":
			await page.getByRole("group", { name: `${step.name} actions`, exact: true }).waitFor();
			await page.getByRole("group", { name: `${step.name} actions`, exact: true }).locator('xpath=ancestor::*[@data-slot="card"]').getByText(step.summary, { exact: true }).waitFor();
			break;
		case "nameWorkspace": await page.locator("#workspace-name").fill(step.value); break;
		case "selectWorkspaceContext":
			await page.getByRole("combobox").click();
			await page.getByRole("option", { name: step.context, exact: true }).click();
			break;
		case "toggleWorkspaceNamespace": await page.getByRole("checkbox", { name: step.namespace, exact: true }).click(); break;
		case "createWorkspace": await page.getByRole("button", { name: "Create workspace", exact: true }).click(); await page.waitForURL(/#\/workspace\/[^/]+\/overview$/); break;
		case "expectOpenWorkspace":
			await page.locator("div.min-w-0").filter({ has: page.getByText(step.name, { exact: true }) }).filter({ has: page.getByText(step.context, { exact: true }) }).first().waitFor();
			break;
		case "selectRbacCategory": await rbacCategory(page, step.category).click(); break;
		case "expectRbacCategory": await rbacCategory(page, step.category).and(page.locator('[aria-pressed="true"]')).waitFor(); break;
		case "filterRbacRisk": await page.getByRole("button", { name: new RegExp(`^${literal(step.filter)} \\d+$`) }).click(); break;
		case "expectRbacItem": await rbacItem(page, step.title).getByText(step.risk, { exact: true }).waitFor(); break;
		case "expectNoRbacItem": if (await rbacItem(page, step.title).count()) throw new Error(`${step.title} is listed`); break;
		case "selectRbacItem": await rbacItem(page, step.title).click(); break;
		case "investigateRbacSubject":
			await page.getByRole("button").filter({ hasText: "Investigate" }).filter({ has: page.locator("span", { hasText: new RegExp(`^${literal(step.subject)}$`) }) }).click();
			break;
		case "expectRbacIdentity": await rbacSection(page, "Verify exact action").getByText(step.identity, { exact: true }).waitFor(); break;
		case "expectRbacGrant": await rbacGrant(page, step.grant).waitFor(); break;
		case "expectNoRbacGrant": if (await rbacGrant(page, step.grant).count()) throw new Error(`${step.grant} is shown`); break;
		case "verifyRbacAccess":
			await page.getByRole("textbox", { name: "Verb", exact: true }).fill(step.verb);
			await page.getByRole("button", { name: "Verify access", exact: true }).click();
			break;
		case "expectRbacVerdict": {
			const label = step.outcome === "noOpinion" ? "No opinion" : step.outcome;
			await rbacSection(page, "Latest verification").locator("p.capitalize", { hasText: new RegExp(`^${label}$`) }).waitFor();
			break;
		}
		case "editRbacReviewNote": await page.getByRole("textbox", { name: "RBAC review note", exact: true }).fill(step.value); break;
		case "expectRbacReviewNote": {
			const input = page.getByRole("textbox", { name: "RBAC review note", exact: true });
			if (await input.inputValue() !== step.value) throw new Error("RBAC review note belongs to another object");
			break;
		}
		case "saveRbacReview": await page.getByRole("button", { name: `Mark ${step.disposition}`, exact: true }).click(); break;
		case "expectRbacReview": await page.getByText("Reviewed context", { exact: true }).locator("xpath=ancestor::section[1]").getByText(step.label, { exact: true }).waitFor(); break;
		case "clearRbacReview": await page.getByRole("button", { name: "Clear review", exact: true }).click(); break;
		case "openSettings": await page.getByRole("button", { name: "Open settings", exact: true }).click(); break;
		case "closeSettings": await page.getByRole("button", { name: "Back to app", exact: true }).click(); break;
		case "showSettingsSection": await page.getByRole("navigation", { name: "Settings sections" }).getByRole("button", { name: step.section, exact: true }).click(); break;
		case "searchSettings": await page.getByRole("textbox", { name: "Search settings", exact: true }).fill(step.value); break;
		case "expectSettingsHeading": await page.getByRole("heading", { level: 2, name: step.heading, exact: true }).waitFor(); break;
		case "expectSettingRow": await page.getByText(step.title, { exact: true }).first().waitFor(); break;
		case "expectNoSettingRow": if (await page.getByText(step.title, { exact: true }).count()) throw new Error(`${step.title} is shown`); break;
		case "expectSettingsEmpty": await page.getByText(`No settings match "${step.value}".`, { exact: true }).waitFor(); break;
		case "addServerKubeconfigPath": case "clickKubeconfigPathButton": case "expectKubeconfigPathBefore": case "expectAccessibleName": await kubeconfigSettingsAction(page, step); break;
		case "setKubeconfigEnvVar": {
			await page.getByRole("textbox", { name: "Kubeconfig env var", exact: true }).fill(step.value);
			const save = page.getByRole("button", { name: "Save", exact: true });
			await save.click();
			await save.and(page.locator(":enabled")).waitFor();
			break;
		}
		case "expectKubeconfigEnvVar": {
			const input = await page.getByRole("textbox", { name: "Kubeconfig env var", exact: true }).elementHandle();
			await page.waitForFunction(({ input, value }) => input instanceof HTMLInputElement && input.value === value, { input, value: step.value });
			break;
		}
		case "selectSettingOption": await page.getByText(step.setting, { exact: true }).locator("xpath=../..").getByRole("radio", { name: step.value, exact: true }).click(); break;
        case "expectSettingOption": {
            const button = page.getByText(step.setting, { exact: true }).locator("xpath=../..").getByRole("radio", { name: step.value, exact: true });
            if (await button.getAttribute("aria-checked") !== "true") throw new Error("Setting option not selected");
            break;
        }
        case "moveResourceKindEarlier": await page.getByRole("button", { name: `${step.kind}, Show first`, exact: true }).press("ArrowUp"); break;
        case "expectResourceKindPosition": {
            const item = page.getByRole("region", { name: step.lane, exact: true }).getByRole("button").nth(step.index);
            if (await item.getAttribute("aria-label") !== `${step.kind}, ${step.lane}`) throw new Error("Resource kind order changed");
            break;
        }
        case "restartClient": await page.reload(); await page.getByRole("button", { name: "Open settings", exact: true }).waitFor(); break;
        case "toggleSetting": await page.getByRole("switch", { name: step.setting, exact: true }).click(); break;
		case "expectSetting": await page.getByRole("switch", { name: step.setting, exact: true }).and(page.locator(`[aria-checked="${step.checked}"]`)).waitFor(); break;
		case "openPalette": {
			// The palette's window shortcut listener mounts with the header's search button.
			const button = page.getByRole("button", { name: "Search views, namespaces, and resources", exact: true });
			await button.waitFor();
			if (step.via === "shortcut") await page.keyboard.press("ControlOrMeta+K");
			else await button.click();
			await page.getByRole("dialog").waitFor();
			break;
		}
		case "searchPalette": await page.getByPlaceholder("Search views, namespaces, and resources...").fill(step.value); break;
		case "expectPaletteResource": await paletteResource(page, step).waitFor(); break;
		case "expectNoPaletteResource": if (await paletteResource(page, step).count()) throw new Error(`${step.name} is a palette result`); break;
		case "choosePaletteResource": await paletteResource(page, step).click(); break;
		case "expectPaletteEntry": await page.getByRole("option", { name: step.label, exact: true }).waitFor(); break;
		case "expectNoPaletteEntry": if (await page.getByRole("option", { name: step.label, exact: true }).count()) throw new Error(`${step.label} is a palette result`); break;
		case "choosePaletteEntry": await page.getByRole("option", { name: step.label, exact: true }).click(); break;
		case "openGitOps":
			await page.getByRole("button", { name: "Open GitOps", exact: true }).scrollIntoViewIfNeeded();
			await page.getByRole("button", { name: "Open GitOps", exact: true }).click(); break;
		case "openGitOpsResources": {
			await page.getByRole("button", { name: `View resources for ${step.name}`, exact: true }).first().click();
			break;
		}
		case "expectArgoWorkspace": await page.getByRole("heading", { level: 2 }).filter({ hasText: step.name }).waitFor(); break;
		case "filterArgoResources":
			await page.getByLabel("Argo CD resource filters").getByRole("button", { name: new RegExp(`^${literal(step.filter)} `) }).click(); break;
		case "expectDeliveryView": await page.getByRole("radio", { name: step.mode, exact: true }).and(page.locator('[aria-checked="true"]')).waitFor(); break;
        case "setDeliveryView": await page.getByRole("radio", { name: step.mode, exact: true }).click(); break;
		// GitOps rows render above the namespace sections, so a shared label such as Workloads matches the GitOps node first.
		case "expandNav": await page.getByRole("button", { name: `Expand ${step.page}`, exact: true }).first().click(); break;
		case "expectGitOpsTable":
			await page.getByRole("heading", { level: 2, name: step.title, exact: true }).waitFor();
			await page.getByText(itemCount(step.count), { exact: true }).waitFor();
			break;
		case "expectGitOpsItem": await gitOpsDetails(page, step).waitFor(); break;
		// Absence is checked after a preceding step waited for the page's rows.
		case "expectNoGitOpsItem": if (await gitOpsDetails(page, step).count()) throw new Error(`${step.kind} ${step.name} is still listed`); break;
		case "openGitOpsDetails": {
			const button = gitOpsDetails(page, step);
			await revealGitOps(page, button);
			await button.click(); break;
		}
		case "expectArgoApplication": {
			const panel = page.getByRole("tabpanel");
			await panel.getByRole("heading", { level: 2, name: step.name, exact: true }).waitFor();
			await panel.getByText(`Health ${step.health}`, { exact: true }).waitFor();
			await panel.getByText(`Sync ${step.sync}`, { exact: true }).waitFor();
			break;
		}
		case "expectArgoSignal":
			await page.getByRole("tabpanel").locator("div.p-2\\.5").filter({ has: page.getByText(step.label, { exact: true }) }).locator("div.text-base").filter({ hasText: new RegExp(`^${step.count}$`) }).waitFor();
			break;
		case "expectHelmCount": await page.getByText(itemCount(step.count), { exact: true }).waitFor(); break;
		case "expectHelmRelease": await helmCard(page, step.name, step.namespace).waitFor(); break;
		case "expectNoHelmRelease": if (await helmCard(page, step.name, step.namespace).count()) throw new Error(`Helm release ${step.name} is still listed`); break;
		case "openHelmRelease": await helmCard(page, step.name, step.namespace).getByRole("button", { name: "Details", exact: true }).click(); break;
		case "selectHelmNamespace": await page.getByRole("button", { name: new RegExp(`^${literal(step.namespace)}\\s*\\d+$`) }).click(); break;
		case "searchHelm": await page.getByRole("textbox", { name: "Search Helm releases" }).fill(step.value); break;
		case "expectHelmDetails": {
			const card = page.locator('[data-slot="card"]').filter({ has: page.getByRole("button", { name: "Close Helm details", exact: true }) });
			await card.locator('[data-slot="card-title"]').getByText(step.title, { exact: true }).waitFor();
			await card.getByText(step.description, { exact: true }).waitFor();
			await card.getByText(step.totals, { exact: true }).waitFor();
			break;
		}
		default: { const unhandled: never = step; throw new Error(`Unhandled parity step: ${JSON.stringify(unhandled)}`); }
	}
}

type Identity = { kind: string; name: string; namespace: string };
function rbacCategory(page: Page, category: string) {
	return page.locator('[aria-label="RBAC categories"]').getByRole("button", { name: new RegExp(`^${literal(category)} \\d+$`) });
}
/** A review queue card; its first line is `Kind/name`. */
function rbacItem(page: Page, title: string) {
	return page.locator("button[aria-pressed]").filter({ has: page.locator("p", { hasText: new RegExp(`^${literal(title)}$`) }) });
}
function rbacSection(page: Page, heading: string) {
	return page.locator("section").filter({ has: page.getByText(heading, { exact: true }) });
}
function rbacGrant(page: Page, grant: string) {
	return rbacSection(page, "Observed RBAC grants").locator("p.font-mono", { hasText: new RegExp(`^${literal(grant)}$`) });
}
/** A palette resource result: its name, then cluster, namespace and kind. */
function paletteResource(page: Page, step: Identity) {
	const row = page.getByRole("option").filter({ has: page.locator("span.truncate", { hasText: new RegExp(`^${literal(step.name)}$`) }) }).filter({ has: page.getByText(step.kind, { exact: true }) });
	return step.namespace ? row.filter({ has: page.getByText(step.namespace, { exact: true }) }) : row;
}

/** Event rows in the Events tab: each starts with its `<time>`. */
/** Waits poll synchronous predicates: Playwright treats a Promise returned by a waitForFunction predicate as truthy without awaiting it. */
async function loadSettingsMocks(page: Page) {
	await page.evaluate("import('/src/lib/settings-integration-mocks.ts').then(m => { globalThis.paritySettingsMocks = m; })");
}
function eventRows(page: Page) { return page.locator("div.grid:has(> time[datetime])"); }
function literal(text: string) { return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"); }
function itemCount(count: number) { return `${count} item${count === 1 ? "" : "s"}`; }
/** A GitOps card or list row, by the label of its Details button. */
function gitOpsDetails(page: Page, item: { kind: string; name: string; namespace: string }) {
	return page.getByRole("button", { name: `Open details for ${item.kind} ${item.name} in ${item.namespace}`, exact: true });
}
/** Scrolls virtual rows until the requested Application mounts or both ends are reached. */
async function revealGitOps(page: Page, target: Locator) {
	await page.getByRole("heading", { level: 2 }).first().waitFor();
	for (const direction of [1, -1]) {
		while (await target.count() === 0) {
			const moved = await page.locator("[data-virtual-row]").first().evaluate(async (row, direction) => {
				const viewport = row.closest(".overflow-y-auto, .overflow-auto");
				if (!(viewport instanceof HTMLElement)) throw new Error("GitOps scroll viewport missing");
				const previous = viewport.scrollTop;
				viewport.scrollTop += direction * viewport.clientHeight;
				await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
				return viewport.scrollTop !== previous;
			}, direction);
			if (!moved) break;
		}
		if (await target.count()) return;
	}
	throw new Error("GitOps details target missing after scrolling the collection");
}
/** A Helm release card, by its title and namespace description. */
function helmCard(page: Page, name: string, namespace: string) {
	return page.locator('[data-slot="card"]')
		.filter({ has: page.locator('[data-slot="card-title"]').getByText(name, { exact: true }) })
		.filter({ has: page.locator('[data-slot="card-description"]').getByText(namespace, { exact: true }) });
}
/** A revision in the Revisions list; its comparison picker shows the same label. */
function revisionButton(page: Page, label: string) { return page.locator("button.w-full.p-3").filter({ has: page.locator("div.font-medium", { hasText: new RegExp(`^${literal(label)}$`) }) }); }
/** Waits until the element's rendered lines include every expected diff line. */
async function containsLines(locator: Locator, lines: string[]) {
	const element = await locator.first().elementHandle();
	await locator.page().waitForFunction(({ element, expected }) => {
		// biome-ignore lint/performance/useDomNodeTextContent: the YAML dry-run diff renders each line as a block <div>, so only rendered text keeps one line per row.
		const rendered = element instanceof HTMLElement ? element.innerText.split("\n") : [];
		return expected.every((line) => rendered.includes(line));
	}, { element, expected: lines });
}

type WebResult = { id: string; status: "passed" | "failed"; durationMs?: number; error?: string };
type Browser = Awaited<ReturnType<typeof chromium.launch>>;

async function runScenario(browser: Browser, port: number, artifacts: string, scenarioIndex: number, scenario: Contract["scenarios"][number], signal: AbortSignal): Promise<WebResult> {
	// Each page has its own browser context, so its storage and in-memory mocks stay isolated.
	const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
	const browserLog: string[] = [];
	page.on("console", (message) => browserLog.push(`${message.type()}: ${message.text()}`));
	page.on("pageerror", (error) => browserLog.push(`pageerror: ${error.message}`));
	page.setDefaultTimeout(readinessTimeoutMs);
	try {
		signal.throwIfAborted();
		// Fake timers must exist before the app loads: the Argo tracker's recovery timer and loading indicator delays start before later steps.
		if (scenario.steps.some((step) => step.action === "controlClock" || step.action === "advanceClock")) await page.clock.install();
		const coverage = await startCodeCoverage(page);
		await page.goto(`http://127.0.0.1:${port}/`);
		const prelude = scenario.steps.findIndex((step) => step.action !== "mockTiming" || step.control.operation !== "hold");
		const startupSteps = prelude < 0 ? scenario.steps.length : prelude;
		for (const step of scenario.steps.slice(0, startupSteps)) await action(page, step);
		if (seedsWorkspace(scenario)) await seedWorkspace(page);
		for (const [index, step] of scenario.steps.entries()) {
			signal.throwIfAborted();
			if (index >= startupSteps) { await storeReloadHolds(page, scenario.steps, index); await action(page, step); }
			await page.screenshot({ path: join(artifacts, `web-${scenarioIndex}-${index}.png`) });
		}
		await coverage?.save(artifacts, scenarioIndex, scenario.id);
		return { id: scenario.id, status: "passed" };
	} catch (error) {
		await page.screenshot({ path: join(artifacts, `web-${scenarioIndex}-failure.png`) }).catch(() => {});
		return { id: scenario.id, status: "failed", error: String(error) };
	} finally {
		await writeFile(join(artifacts, `web-${scenarioIndex}.log`), browserLog.join("\n"));
		await page.close();
	}
}

/** Runs scenarios on `jobs` pages at once; results keep the contract order. */
export async function runWeb(contract: Contract, artifacts: string, signal: AbortSignal, jobs = 1) {
	const results: WebResult[] = [];
	const port = await freePort();
	const env = { ...process.env }; delete env.KUBECONFIG; delete env.ARGV0;
	const vite = Bun.spawn([process.execPath, join(process.cwd(), "node_modules/vite/bin/vite.js"), "--host", "127.0.0.1", "--port", String(port), "--strictPort"], { cwd: process.cwd(), env, detached: true, stdout: "pipe", stderr: "pipe" });
	const identity = startIdentity(vite.pid);
	const logs = Promise.all([new Response(vite.stdout).text(), new Response(vite.stderr).text()]);
	let browser: Browser | undefined;
	const cancel = () => { void browser?.close(); };
	signal.addEventListener("abort", cancel, { once: true });
	try {
		let abortReadiness = () => {};
		try {
			await Promise.race([
				waitFor("Vite", async (timeout) => (await fetch(`http://127.0.0.1:${port}/`, { signal: timeout })).ok),
				new Promise<never>((_, reject) => { abortReadiness = () => reject(signal.reason); signal.addEventListener("abort", abortReadiness, { once: true }); }),
			]);
		} finally { signal.removeEventListener("abort", abortReadiness); }
		signal.throwIfAborted();
		const launched = await chromium.launch({ executablePath: chromeExecutable(), headless: true });
		browser = launched;
		signal.throwIfAborted();
		const ordered: (WebResult | undefined)[] = [];
		let next = 0;
		const worker = async () => {
			while (next < contract.scenarios.length && !signal.aborted) {
				const scenarioIndex = next++;
				const scenario = contract.scenarios[scenarioIndex];
				if (!scenario) continue;
				const started = performance.now();
				const result = await runScenario(launched, port, artifacts, scenarioIndex, scenario, signal);
				ordered[scenarioIndex] = { ...result, durationMs: Math.round(performance.now() - started) };
			}
		};
		await Promise.all(Array.from({ length: Math.max(1, Math.min(jobs, contract.scenarios.length)) }, worker));
		for (const result of ordered) if (result) results.push(result);
	} finally {
		signal.removeEventListener("abort", cancel);
		await browser?.close();
		const stopped = await stopGroup(vite.pid, identity);
		if (!stopped) throw new Error(`Could not stop owned Vite process group ${vite.pid}`);
		await writeFile(join(artifacts, "vite.log"), (await logs).join("\n"));
		await writeFile(join(artifacts, "web.json"), JSON.stringify({ driver: "web", scenarios: results }, null, 2));
	}
}
