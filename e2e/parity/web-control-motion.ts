import type { Locator, Page } from "playwright-core";
import { resourceRow } from "./web-resource-table";
import { sidebarRow } from "./web-sidebar-live";

/** Tailwind's `--default-transition-duration`, which every control below transitions over. */
const DURATION_MS = 150;

declare global {
	interface Window {
		controlMotion?: Animation[];
		/** Whether the followed property started a transition in an `instant` check. */
		controlMotionRan?: boolean;
	}
}

type Target = {
	/** The element whose transition is followed. */
	moving(page: Page): Locator;
	/** The pseudo-element that carries the followed property, if it is not the element itself. */
	pseudo?: string;
	/** The computed property the transition changes. */
	property: string;
	/** What the page shows, where the followed property alone does not say it. */
	shown?(page: Page): Promise<string>;
	/** Throws unless the indicator covers the chosen option exactly once it has settled. */
	fits?(page: Page): Promise<void>;
	/** Changes the control's state. */
	act(page: Page): Promise<void>;
	/** Chooses the first state again, and the element whose transition then runs if it is another. */
	back?: { moving?(page: Page): Locator; act(page: Page): Promise<void> };
};

const deployment = { kind: "Deployment", name: "payments-api", namespace: "payments" };

const TARGETS = {
	// Switch.svelte: the thumb's `transition-transform` to `translate-x-4`.
	switch: {
		moving: (page) => page.getByRole("switch", { name: "Show exact timestamps", exact: true }).locator("[data-slot='switch-thumb']"),
		property: "translate",
		act: (page) => page.getByRole("switch", { name: "Show exact timestamps", exact: true }).click(),
	},
	// SegmentedControl.svelte: the pill's `transition-[left,top,width,height] duration-150` to the chosen option.
	segment: {
		moving: (page) => page.getByRole("radiogroup").filter({ has: page.getByRole("radio", { name: "List", exact: true }) }).locator("[data-slot='segmented-control-pill']"),
		property: "left",
		act: (page) => page.getByRole("radio", { name: "List", exact: true }).click(),
		back: { act: (page) => page.getByRole("radio", { name: "Cards", exact: true }).click() },
		// Options are fractional pixels wide, so a surface measured in whole pixels misses one by a
		// pixel. The pill's last `transitionend` reaches the page with a later frame, so this waits.
		fits: async (page) => {
			const group = await page.getByRole("radiogroup").filter({ has: page.getByRole("radio", { name: "List", exact: true }) }).elementHandle();
			const fitted = await page.waitForFunction((options) => {
				const chosen = options.querySelector("[aria-checked='true']");
				if (!chosen) return false;
				const pill = options.querySelector("[data-slot='segmented-control-pill']");
				const surfaces = [pill, chosen].filter((item): item is Element => {
					if (!item) return false;
					const style = getComputedStyle(item);
					return style.visibility === "visible" && style.backgroundColor !== "rgba(0, 0, 0, 0)";
				});
				const edges = (item: Element) => {
					const box = item.getBoundingClientRect();
					return [box.left, box.top, box.right, box.bottom].map((edge) => edge.toFixed(2)).join(" ");
				};
				const [surface] = surfaces;
				return surfaces.length === 1 && surface !== undefined && edges(surface) === edges(chosen);
			}, group).then(() => true, () => false);
			if (!fitted) throw new Error("At rest, not exactly one surface covers the chosen option edge to edge");
		},
	},
	// TabsTrigger.svelte: the line's `after:transition-[translate,scale]` from the last active tab.
	// The new tab's line starts offset and scaled onto the old tab, so the page shows the line's
	// left edge and width: the active tab's padding box moved by that offset and scaled.
	tab: {
		moving: (page) => page.getByRole("tab", { name: "Events", exact: true }),
		pseudo: "::after",
		property: "translate",
		shown: (page) => page.getByRole("tab", { name: "Events", exact: true }).evaluate((tab) => {
			const active = tab.closest("[role='tablist']")?.querySelector("[role='tab'][data-state='active']");
			if (!active) throw new Error("No active tab");
			const line = getComputedStyle(active, "::after");
			const [x = 0] = line.translate === "none" ? [] : line.translate.split(" ").map(Number.parseFloat);
			const [w = 1] = line.scale === "none" ? [] : line.scale.split(" ").map(Number.parseFloat);
			const box = active.getBoundingClientRect();
			const style = getComputedStyle(active);
			const [left, right] = [style.borderLeftWidth, style.borderRightWidth].map(Number.parseFloat);
			// Hundredths of a pixel: the sum of two edges does not round as one edge does.
			return `${(box.left + left + x).toFixed(2)} ${((box.width - left - right) * w).toFixed(2)}`;
		}),
		// At rest only the active tab shows its line, unmoved and unscaled under it.
		fits: async (page) => {
			const fitted = await page.getByRole("tab", { name: "Events", exact: true }).evaluate((tab) => {
				const tabs = [...(tab.closest("[role='tablist']")?.querySelectorAll("[role='tab']") ?? [])];
				const lined = tabs.filter((item) => getComputedStyle(item, "::after").opacity !== "0");
				const [only] = lined;
				if (lined.length !== 1 || only?.getAttribute("data-state") !== "active") return false;
				const line = getComputedStyle(only, "::after");
				return ["none", "0px", "0px 0px"].includes(line.translate) && ["none", "1", "1 1"].includes(line.scale);
			});
			if (!fitted) throw new Error("At rest, not exactly the active tab shows its line in place");
		},
		act: (page) => page.getByRole("tab", { name: "Events", exact: true }).click(),
		// The line then runs on the first tab, from where the second tab's line is shown.
		back: {
			moving: (page) => page.getByRole("tab", { name: "Details", exact: true }),
			act: (page) => page.getByRole("tab", { name: "Details", exact: true }).click(),
		},
	},
	// TableRow.svelte: `transition-colors` to the hover background.
	row: {
		moving: (page) => resourceRow(page, deployment),
		property: "background-color",
		act: (page) => resourceRow(page, deployment).hover(),
	},
	// buttonVariants in classes.ts: `transition-[color,background-color,…]` to the variant's hover background.
	button: {
		moving: (page) => page.getByRole("button", { name: "Edit YAML", exact: true }),
		property: "background-color",
		act: (page) => page.getByRole("button", { name: "Edit YAML", exact: true }).hover(),
	},
	// SidebarTreeNode.svelte: the chevron's `transition-transform` to `rotate-90`.
	chevron: {
		moving: (page) => sidebarRow(page, ["Workloads"]).getByRole("button").locator("svg"),
		property: "rotate",
		act: (page) => sidebarRow(page, ["Workloads"]).getByRole("button").click(),
	},
} satisfies Record<string, Target>;

function targetOf(name: string | undefined): Target | undefined {
	return Object.entries(TARGETS).find(([key]) => key === name)?.[1];
}

/** The computed value each page's last check saw, by phase. */
const seen = new WeakMap<Page, { rest: string; midway?: string }>();

/**
 * The followed property's computed value in one form. A running transition serializes it another
 * way than the style at rest does: `oklab(…)` for a color whatever space it was written in, and
 * `0px` or `0deg` where the style at rest reads `none`.
 */
function read(page: Page, target: Target): Promise<string> {
	if (target.shown) return target.shown(page);
	return target.moving(page).evaluate((element, { pseudo, property }) => {
		const value = getComputedStyle(element, pseudo).getPropertyValue(property);
		if (!property.endsWith("color")) return String(value === "none" ? 0 : Number.parseFloat(value));
		const context = new OffscreenCanvas(1, 1).getContext("2d");
		if (!context) throw new Error("No 2D canvas to read a color with");
		context.fillStyle = value;
		context.fillRect(0, 0, 1, 1);
		return context.getImageData(0, 0, 1, 1).data.join(" ");
	}, { pseudo: target.pseudo, property: target.property });
}

/** Moves the page's paused transitions to `ms`, or to their end. */
function seek(page: Page, ms: number | "end") {
	return page.evaluate((to) => {
		for (const animation of window.controlMotion ?? []) {
			if (to === "end") animation.finish();
			else animation.currentTime = to;
		}
	}, ms);
}

/**
 * Pauses the transitions on `moving` at time 0 once `act` starts one of the target's property, so
 * the properties that move together, such as a pill's left and width, stay at the same time.
 */
async function follow(page: Page, moving: Locator, target: Target, act: (page: Page) => Promise<void>) {
	await moving.evaluate((element, { pseudo, property }) => {
		window.controlMotion = undefined;
		const capture = (event: Event) => {
			// Transitions on descendants bubble here too.
			if (event.target !== element) return;
			const transitions = element.getAnimations({ subtree: true }).filter((animation): animation is CSSTransition => {
				if (!(animation instanceof CSSTransition)) return false;
				const effect = animation.effect;
				return effect instanceof KeyframeEffect && effect.target === element && effect.pseudoElement === pseudo;
			});
			// The pointer's move onto the control can start a hover transition of another property a
			// frame before the click starts the followed one. Keep listening until that one runs.
			const followed = transitions.find((transition) => transition.transitionProperty === property);
			if (!followed) return;
			element.removeEventListener("transitionrun", capture);
			for (const transition of transitions) {
				transition.pause();
				transition.currentTime = 0;
			}
			// The timing check reads the followed transition first.
			window.controlMotion = [followed, ...transitions.filter((transition) => transition !== followed)];
		};
		element.addEventListener("transitionrun", capture);
	}, { pseudo: target.pseudo ?? null, property: target.property });
	await act(page);
	await page.waitForFunction(() => window.controlMotion !== undefined).catch(() => { throw new Error(`The control changed without a transition of ${target.property}`); });
}

/**
 * `motion:<target>:<phase>` follows one control's CSS transition. `start` changes the control's
 * state and expects its computed value unchanged at time 0; `midway` expects a value between the
 * ends at half of DURATION_MS; `settled` expects the end value. After `midway`, `back` chooses the
 * first state again and expects the value unchanged, and `returned` expects the value at rest.
 * `instant`, run under reduced motion, changes the state and expects no transition of the
 * followed property and the indicator at rest on the new state.
 *
 * A CSS transition runs on the document timeline, which the page clock does not move. So `start`
 * pauses the control's transitions in their `transitionrun` event, and the later phases seek
 * them. The scenario's `advanceClock` steps move the GPUI driver's clock by the same times.
 */
export async function checkControlMotion(page: Page, check: string) {
	const [, name, phase] = check.split(":");
	const target = targetOf(name);
	if (!target) throw new Error(`Unknown motion target in ${check}`);
	if (phase === "instant") {
		const moving = target.moving(page);
		await moving.waitFor({ state: "attached" });
		const rest = await read(page, target);
		await moving.evaluate((element, { pseudo, property }) => {
			window.controlMotionRan = false;
			element.addEventListener("transitionrun", (event) => {
				if (event instanceof TransitionEvent && event.target === element && event.pseudoElement === pseudo && event.propertyName === property) window.controlMotionRan = true;
			});
		}, { pseudo: target.pseudo ?? "", property: target.property });
		await target.act(page);
		// A transition's `transitionrun` reaches the page with the next frame.
		await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
		if (await page.evaluate(() => window.controlMotionRan)) throw new Error(`${name} moved ${target.property} under reduced motion`);
		if ((await read(page, target)) === rest) throw new Error(`${name} did not change under reduced motion`);
		await target.fits?.(page);
		return;
	}
	if (phase === "start") {
		const moving = target.moving(page);
		// The segmented control's pill is hidden at rest, so this waits for the element only.
		await moving.waitFor({ state: "attached" });
		const rest = await read(page, target);
		await follow(page, moving, target, target.act);
		const timing = await page.evaluate(() => {
			const computed = window.controlMotion?.[0]?.effect?.getComputedTiming();
			return { duration: Number(computed?.duration ?? -1), delay: computed?.delay ?? -1 };
		});
		if (timing.duration !== DURATION_MS || timing.delay !== 0) throw new Error(`${name} transitions ${target.property} over ${timing.duration} ms after ${timing.delay} ms, not over ${DURATION_MS} ms at once`);
		const held = await read(page, target);
		if (held !== rest) throw new Error(`${name} jumped from ${rest} to ${held}`);
		seen.set(page, { rest });
		return;
	}
	const before = seen.get(page);
	if (!before) throw new Error(`${check} needs an earlier start check`);
	if (phase === "midway") {
		await seek(page, DURATION_MS / 2);
		const midway = await read(page, target);
		if (midway === before.rest) throw new Error(`${name} has not moved from ${before.rest} at ${DURATION_MS / 2} ms`);
		seen.set(page, { ...before, midway });
		return;
	}
	if (phase === "back") {
		if (!target.back || before.midway === undefined) throw new Error(`${check} needs a target that turns back and an earlier midway check`);
		await follow(page, (target.back.moving ?? target.moving)(page), target, target.back.act);
		// The replaced transition's `transitioncancel` reaches the page with the next frame, and the
		// page may answer it. The followed transition is paused, so waiting moves nothing.
		await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
		const held = await read(page, target);
		if (held !== before.midway) throw new Error(`${name} jumped from ${before.midway} to ${held} when its first state was chosen again`);
		return;
	}
	if (phase === "returned") {
		await seek(page, "end");
		const returned = await read(page, target);
		if (returned !== before.rest) throw new Error(`${name} returned to ${returned}, not to ${before.rest} at rest`);
		await target.fits?.(page);
		return;
	}
	if (phase !== "settled") throw new Error(`Unknown motion phase in ${check}`);
	await seek(page, "end");
	const settled = await read(page, target);
	if (settled === before.rest || settled === before.midway) throw new Error(`${name} ended at ${settled}, its value ${settled === before.rest ? "at rest" : "midway"}`);
	await target.fits?.(page);
}
