// How `controlEase` could fail, written before the function:
// 1. It returns the curve's y at the parameter equal to the time, without solving the curve's x
//    for that time. That value at half time is 0.5 for this curve, and a browser shows 0.775561.
// 2. Its ends are near 0 and 1 but not equal to them, so a branch opens a fraction short.
// 3. The solver does not converge for some time, so the output is not a number, leaves 0 to 1,
//    or steps back between two of the keyframes Svelte samples.
import { expect, test } from "bun:test";
import { controlEase } from "./controlEase";

test("matches the browser's cubic-bezier(0.4, 0, 0.2, 1) at half time", () => {
	// WebKitGTK 2.54.2 moved a 150 ms `translate` transition on this curve from -57.4375 px to
	// -12.891197 px at 75 ms: 1 - 12.891197 / 57.4375. The measurement has six decimals.
	expect(controlEase(0.5)).toBeCloseTo(0.775561, 5);
});

test("starts at exactly 0 and ends at exactly 1", () => {
	expect(controlEase(0)).toBe(0);
	expect(controlEase(1)).toBe(1);
});

test("rises through every keyframe Svelte samples for 150 ms", () => {
	// Svelte's `animate` builds one keyframe per 60 Hz frame: `Math.ceil(150 / (1000 / 60))` steps.
	const steps = Math.ceil(150 / (1000 / 60));
	const values = Array.from({ length: steps + 1 }, (_, step) => controlEase(step / steps));
	for (const [step, value] of values.entries()) {
		expect(value).toBeGreaterThanOrEqual(0);
		expect(value).toBeLessThanOrEqual(1);
		if (step > 0) expect(value).toBeGreaterThan(values[step - 1]);
	}
});
