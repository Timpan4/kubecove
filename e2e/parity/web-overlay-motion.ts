import type { Page } from "playwright-core";
import type { Step } from "./contract";
import type { OverlayName } from "./contract-steps";

type OverlayMotionStep = Extract<Step, { action: "holdOverlayMotion" | "advanceOverlayMotion" | "expectOverlayOpacity" | "expectNoOverlay" }>;

/** The element each overlay name draws on, by the shared wrappers' `data-slot`. */
const SURFACES = {
	"update-status": '[data-slot="popover-content"]',
} satisfies Record<OverlayName, string>;
/** The scenario gives opacities to four decimals, and gpui-base's `cubic_bezier` tests bound its distance from CSS at 5e-4. */
const OPACITY_TOLERANCE = 6e-4;

/**
 * Follows the wrappers' `enter` and `exit` keyframe animations (tw-animate-css). They run on the
 * compositor's clock, which `page.clock` does not move, so `holdOverlayMotion` pauses every CSS
 * animation: each then starts on its first keyframe and moves only when `advanceOverlayMotion`
 * seeks it. A paused animation that is sought to its end stays paused and fires no
 * `animationend`, so the seek finishes it instead, which lets bits-ui unmount a closed overlay.
 */
export async function overlayMotionAction(page: Page, step: OverlayMotionStep): Promise<void> {
	switch (step.action) {
		case "holdOverlayMotion":
			await page.addStyleTag({ content: "*, *::before, *::after { animation-play-state: paused !important; }" });
			break;
		case "advanceOverlayMotion":
			await page.evaluate((ms) => {
				for (const animation of document.getAnimations()) {
					if (!(animation instanceof CSSAnimation) || !["enter", "exit"].includes(animation.animationName)) continue;
					const next = Number(animation.currentTime ?? 0) + ms;
					const end = Number(animation.effect?.getComputedTiming().endTime ?? 0);
					if (next >= end) animation.finish();
					else animation.currentTime = next;
				}
			}, step.ms);
			break;
		case "expectOverlayOpacity": {
			const surface = page.locator(SURFACES[step.overlay]);
			await surface.waitFor({ state: "attached" });
			try {
				await page.waitForFunction(
					({ selector, opacity, tolerance }) => {
						const element = document.querySelector(selector);
						return element !== null && Math.abs(Number(getComputedStyle(element).opacity) - opacity) <= tolerance;
					},
					{ selector: SURFACES[step.overlay], opacity: step.opacity, tolerance: OPACITY_TOLERANCE },
				);
			} catch {
				const drawn = await surface.evaluate((element) => getComputedStyle(element).opacity);
				throw new Error(`${step.overlay} drew at opacity ${drawn}, not ${step.opacity}`);
			}
			break;
		}
		case "expectNoOverlay":
			await page.locator(SURFACES[step.overlay]).waitFor({ state: "detached" });
			break;
	}
}
