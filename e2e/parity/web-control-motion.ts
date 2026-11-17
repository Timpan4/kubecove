import type { Locator, Page } from "playwright-core";
import { resourceRow } from "./web-resource-table";
import { sidebarRow } from "./web-sidebar-live";

/** Tailwind's `--default-transition-duration`, which every control below transitions over. */
const DURATION_MS = 150;

declare global {
	interface Window { controlMotion?: Animation[] }
}

type Target = {
	/** The element whose transition is followed. */
	moving(page: Page): Locator;
	/** The pseudo-element that carries the followed property, if it is not the element itself. */
	pseudo?: string;
	/** The computed property the transition changes. */
	property: string;
	/** Changes the control's state. */
	act(page: Page): Promise<void>;
};

const deployment = { kind: "Deployment", name: "payments-api", namespace: "payments" };

const TARGETS = {
	// Switch.svelte: the thumb's `transition-transform` to `translate-x-4`.
	switch: {
		moving: (page) => page.getByRole("switch", { name: "Show exact timestamps", exact: true }).locator("[data-slot='switch-thumb']"),
		property: "translate",
		act: (page) => page.getByRole("switch", { name: "Show exact timestamps", exact: true }).click(),
	},
	// SegmentedControl.svelte: the option's `transition-[color,background-color,…] duration-150`.
	segment: {
		moving: (page) => page.getByRole("radio", { name: "List", exact: true }),
		property: "background-color",
		act: (page) => page.getByRole("radio", { name: "List", exact: true }).click(),
	},
	// TabsTrigger.svelte: the line's `after:transition-opacity`.
	tab: {
		moving: (page) => page.getByRole("tab", { name: "Events", exact: true }),
		pseudo: "::after",
		property: "opacity",
		act: (page) => page.getByRole("tab", { name: "Events", exact: true }).click(),
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
 * `motion:<target>:<phase>` follows one control's CSS transition. `start` changes the control's
 * state and expects its computed value unchanged at time 0; `midway` expects a value between the
 * ends at half of DURATION_MS; `settled` expects the end value.
 *
 * A CSS transition runs on the document timeline, which the page clock does not move. So `start`
 * pauses the control's transitions in their `transitionrun` event, and the later phases seek
 * them. The scenario's `advanceClock` steps move the GPUI driver's clock by the same times.
 */
export async function checkControlMotion(page: Page, check: string) {
	const [, name, phase] = check.split(":");
	const target = targetOf(name);
	if (!target) throw new Error(`Unknown motion target in ${check}`);
	if (phase === "start") {
		const moving = target.moving(page);
		await moving.waitFor();
		const rest = await read(page, target);
		await moving.evaluate((element, { pseudo, property }) => {
			window.controlMotion = undefined;
			const capture = (event: Event) => {
				// Transitions on descendants bubble here too.
				if (event.target !== element) return;
				const transitions = element.getAnimations({ subtree: true }).filter((animation) => {
					if (!(animation instanceof CSSTransition) || animation.transitionProperty !== property) return false;
					const effect = animation.effect;
					return effect instanceof KeyframeEffect && effect.target === element && effect.pseudoElement === pseudo;
				});
				// The pointer's move onto the control can start a hover transition of another property a
				// frame before the click starts the followed one. Keep listening until that one runs.
				if (transitions.length === 0) return;
				element.removeEventListener("transitionrun", capture);
				for (const transition of transitions) {
					transition.pause();
					transition.currentTime = 0;
				}
				window.controlMotion = transitions;
			};
			element.addEventListener("transitionrun", capture);
		}, { pseudo: target.pseudo ?? null, property: target.property });
		await target.act(page);
		await page.waitForFunction(() => window.controlMotion !== undefined).catch(() => { throw new Error(`${name} changed without a transition of ${target.property}`); });
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
	if (phase !== "settled") throw new Error(`Unknown motion phase in ${check}`);
	await seek(page, "end");
	const settled = await read(page, target);
	if (settled === before.rest || settled === before.midway) throw new Error(`${name} ended at ${settled}, its value ${settled === before.rest ? "at rest" : "midway"}`);
}
