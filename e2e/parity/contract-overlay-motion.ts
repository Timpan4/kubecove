import { type StepParsers, count, isNumber, oneOf, noPayload } from "./contract-values";

const OVERLAY_NAMES = ["update-status"] as const;
export type OverlayName = (typeof OVERLAY_NAMES)[number];

export const overlayMotionSteps = {
	holdOverlayMotion: noPayload,
	advanceOverlayMotion: (item) => ({ ms: count(item.ms) }),
	expectOverlayOpacity: (item) => {
		const opacity = item.opacity;
		if (!isNumber(opacity) || !(opacity >= 0 && opacity <= 1)) throw new Error("Expected an opacity from 0 to 1");
		return { overlay: oneOf(OVERLAY_NAMES, item.overlay, "an overlay name"), opacity };
	},
	expectNoOverlay: (item) => ({ overlay: oneOf(OVERLAY_NAMES, item.overlay, "an overlay name") }),
} satisfies StepParsers;
