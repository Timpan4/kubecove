import type { JsonObject } from "../../src/lib/types";
import { OVERLAY_NAMES, type Step } from "./contract-steps";
import { count, isNumber, oneOf } from "./contract-values";

/** Parses overlay motion steps; returns undefined for any other action. */
export function overlayMotionStep(item: JsonObject): Step | undefined {
	switch (item.action) {
		case "holdOverlayMotion": return { action: item.action };
		case "advanceOverlayMotion": return { action: item.action, ms: count(item.ms) };
		case "expectOverlayOpacity": {
			const opacity = item.opacity;
			if (!isNumber(opacity) || !(opacity >= 0 && opacity <= 1)) throw new Error("Expected an opacity from 0 to 1");
			return { action: item.action, overlay: oneOf(OVERLAY_NAMES, item.overlay, "an overlay name"), opacity };
		}
		case "expectNoOverlay": return { action: item.action, overlay: oneOf(OVERLAY_NAMES, item.overlay, "an overlay name") };
		default: return undefined;
	}
}
