import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { $, $$, browser, expect } from "@wdio/globals";
import { beforeEach, describe, it } from "mocha";

type IncidentFocusBounds = Pick<DOMRect, "x" | "y" | "width" | "height" | "top" | "right" | "bottom" | "left">;
interface IncidentFocusEvent {
	kind: string;
	at: number;
	bounds: IncidentFocusBounds;
	connected: boolean;
	sameAction: boolean;
	active: boolean;
	focusVisible: boolean;
	activeElement: { tag: string; text: string | null; data: DOMStringMap } | null;
	viewport: { width: number; height: number; clientHeight: number; visualHeight: number | undefined };
	scrollTop: number;
	scrollHeight: number;
	clientHeight: number;
	scrollerBounds: IncidentFocusBounds;
	guideBounds: IncidentFocusBounds;
	stepCount: number;
	loading: boolean;
	fonts: Document["fonts"]["status"];
}

declare global {
	interface Window {
		copiedIncidentName?: string;
		incidentFocusDiagnostics?: {
			events: IncidentFocusEvent[];
			record: (kind: string) => void;
			cleanup: () => void;
		};
	}
}

describe("Incident Cockpit", () => {
	beforeEach(async () => {
		await browser.url("/");
		await browser.execute(() => localStorage.clear());
		await browser.refresh();
		await $("#workspace-name").setValue("Fast Mock Lab");
		const create = await $("button=Create workspace");
		await create.waitForEnabled();
		await create.click();
	});

	it("reflows compact content without nested scroll panes", async () => {
		const originalSize = await browser.getWindowSize();
		const longResourceName = "opensearch-dashboards-85b6c79f6d-f9x7m";

		try {
			await $('button[aria-label="Open workspace navigation"]').click();
			const navigation = await $('[data-slot="sheet-content"]');
			await navigation.$('[role="treeitem"]*=Incidents').click();
			const layout = await $("[data-incident-layout]");
			const resource = await $(`button*=${longResourceName}`);
			const copy = await $(`button[aria-label="Copy resource name ${longResourceName}"]`);

			for (const [width, height, expectedColumns, expectedOverflow] of [
				[1026, 752, 1, "visible"],
				[1440, 900, 2, "auto"],
			] as const) {
				await browser.sendCommandAndGetResult("Emulation.setDeviceMetricsOverride", {
					width,
					height,
					deviceScaleFactor: 1,
					mobile: false,
				});
				await expect(layout).toBeDisplayed();
				await expect(resource).toBeDisplayed();

				const reflow = await browser.execute((resourceName: string) => {
					const layoutElement = document.querySelector<HTMLElement>("[data-incident-layout]");
					const queueElement = document.querySelector<HTMLElement>("[data-incident-queue-scroll]");
					const guideElement = document.querySelector<HTMLElement>("[data-incident-guide]");
					const resourceButton = Array.from(document.querySelectorAll<HTMLElement>("button")).find(
						(element) => element.textContent?.includes(resourceName),
					);
					if (!layoutElement || !queueElement || !guideElement || !resourceButton) return null;
					const resourceCard = resourceButton.parentElement;
					return {
						columns: getComputedStyle(layoutElement).gridTemplateColumns.split(" ").length,
						guideOverflow: getComputedStyle(guideElement).overflowY,
						queueOverflow: getComputedStyle(queueElement).overflowY,
						resourceFitsCard: (resourceCard?.scrollWidth ?? 1) <= (resourceCard?.clientWidth ?? 0),
						resourceNameVisible: resourceButton.textContent?.includes("Pod/opensearch-dashboards-85b6c79f6d-f9x7m") ?? false,
						statusVisible: resourceButton.textContent?.includes("Needs attention") ?? false,
					};
				}, longResourceName);

				expect(reflow).toEqual({
					columns: expectedColumns,
					guideOverflow: expectedOverflow,
					queueOverflow: expectedOverflow,
					resourceFitsCard: true,
					resourceNameVisible: true,
					statusVisible: true,
				});
			}

			await browser.sendCommandAndGetResult("Emulation.setDeviceMetricsOverride", {
				width: 1026,
				height: 752,
				deviceScaleFactor: 1,
				mobile: false,
			});
			await browser.execute(() => {
				Object.defineProperty(navigator, "clipboard", {
					configurable: true,
					value: {
						writeText: async (text: string) => {
							window.copiedIncidentName = text;
						},
					},
				});
			});
			await copy.click();
			await expect(copy).toHaveText(expect.stringContaining("Copied"));
			expect(
				await browser.execute(
					() => window.copiedIncidentName,
				),
			).toBe(longResourceName);

			await resource.click();
			const keyboardStart = await $("button=Show context");
			const remediation = await $("[data-incident-remediation]");
			await remediation.waitForDisplayed();
			const started = await browser.execute((element: HTMLElement) => {
				const outerScroller = element.closest('[data-slot="scroll-area"]');
				const action = document.querySelector<HTMLElement>("[data-incident-remediation]");
				const guide = document.querySelector<HTMLElement>("[data-incident-guide]");
				if (outerScroller instanceof HTMLElement && action && guide) {
					const events: IncidentFocusEvent[] = [];
					const record = (kind: string) => {
						const active = document.activeElement;
						events.push({
							kind, at: performance.now(),
							bounds: action.getBoundingClientRect().toJSON(),
							connected: action.isConnected,
							sameAction: document.querySelector("[data-incident-remediation]") === action,
							active: active === action, focusVisible: action.matches(":focus-visible"),
							activeElement: active instanceof HTMLElement ? { tag: active.tagName, text: active.textContent, data: { ...active.dataset } } : null,
							viewport: { width: innerWidth, height: innerHeight, clientHeight: document.documentElement.clientHeight, visualHeight: visualViewport?.height },
							scrollTop: outerScroller.scrollTop, scrollHeight: outerScroller.scrollHeight,
							clientHeight: outerScroller.clientHeight, scrollerBounds: outerScroller.getBoundingClientRect().toJSON(),
							guideBounds: guide.getBoundingClientRect().toJSON(),
							stepCount: guide.querySelectorAll("[data-incident-investigation-step]").length,
							loading: guide.textContent?.includes("Loading") ?? false, fonts: document.fonts.status,
						});
					};
					const onScroll = () => record("scroll");
					const onFocus = (event: FocusEvent) => record(event.type);
					const mutations = new MutationObserver(() => record("mutation"));
					const resize = new ResizeObserver(() => record("resize"));
					mutations.observe(guide, { childList: true, subtree: true, characterData: true });
					resize.observe(guide);
					outerScroller.addEventListener("scroll", onScroll);
					guide.addEventListener("focusin", onFocus);
					guide.addEventListener("focusout", onFocus);
					window.incidentFocusDiagnostics = { events, record, cleanup: () => {
						mutations.disconnect();
						resize.disconnect();
						outerScroller.removeEventListener("scroll", onScroll);
						guide.removeEventListener("focusin", onFocus);
						guide.removeEventListener("focusout", onFocus);
					} };
					record("before-focus");
				}
				element.focus();
				if (outerScroller instanceof HTMLElement) outerScroller.scrollTop = 0;
				return document.activeElement === element;
			}, keyboardStart);
			expect(started).toBe(true);

			await browser.keys("Tab");
			const investigationStep = await $("[data-incident-investigation-step]");
			expect(
				await browser.execute(
					(element: HTMLElement) => document.activeElement === element && element.matches(":focus-visible"),
					investigationStep,
				),
			).toBe(true);

			const investigationStepCount = await $$('[data-incident-investigation-step]').length;
			for (let index = 0; index < investigationStepCount; index++) await browser.keys("Tab");
			const readKeyboardResult = () => browser.execute((element: HTMLElement) => {
				window.incidentFocusDiagnostics?.record("assertion");
				const bounds = element.getBoundingClientRect();
				const outerScroller = element.closest('[data-slot="scroll-area"]');
				return {
					active: document.activeElement === element,
					focusVisible: element.matches(":focus-visible"),
					outerScrolled: outerScroller instanceof HTMLElement && outerScroller.scrollTop > 0,
					visible: bounds.top >= 0 && bounds.bottom <= document.documentElement.clientHeight,
				};
			}, remediation);
			await browser.waitUntil(async () => {
				const result = await readKeyboardResult();
				return result.active && result.focusVisible && result.outerScrolled && result.visible;
			}, { timeoutMsg: "Keyboard focus did not scroll the remediation action fully into view." });
			expect(await readKeyboardResult()).toEqual({ active: true, focusVisible: true, outerScrolled: true, visible: true });
		} catch (error) {
			try {
				console.error("Incident keyboard geometry", await browser.execute(() => {
					const element = document.querySelector<HTMLElement>("[data-incident-remediation]");
					const scroller = element?.closest<HTMLElement>('[data-slot="scroll-area"]');
					return {
						viewport: { width: innerWidth, height: innerHeight, clientHeight: document.documentElement.clientHeight, visualHeight: visualViewport?.height },
						bounds: element?.getBoundingClientRect().toJSON(),
						active: document.activeElement === element,
						focusVisible: element?.matches(":focus-visible"),
						scrollTop: scroller?.scrollTop,
						scrollerBounds: scroller?.getBoundingClientRect().toJSON(),
						fonts: document.fonts.status,
					};
				}));
				const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
				if (artifacts) await browser.saveScreenshot(join(artifacts, "incident-before-viewport-reset.png"));
			} catch (diagnosticError) {
				console.error("Could not retain Incident failure screenshot", diagnosticError);
			}
			throw error;
		} finally {
			try {
				const events = await browser.execute(() => {
					const diagnostics = window.incidentFocusDiagnostics;
					diagnostics?.record("before-viewport-reset");
					diagnostics?.cleanup();
					delete window.incidentFocusDiagnostics;
					return diagnostics?.events ?? [];
				});
				const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
				if (artifacts) await writeFile(join(artifacts, "incident-focus-trace.json"), JSON.stringify(events, null, 2));
			} catch (diagnosticError) {
				console.error("Could not retain Incident focus trace", diagnosticError);
			}
			await browser.sendCommandAndGetResult("Emulation.clearDeviceMetricsOverride", {});
			await browser.setWindowSize(originalSize.width, originalSize.height);
		}
	});
});
