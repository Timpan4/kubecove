// Tailwind's `--default-transition-timing-function`, `cubic-bezier(0.4, 0, 0.2, 1)`.
const [X1, Y1, X2, Y2] = [0.4, 0, 0.2, 1];

/** One coordinate of the curve at parameter `s`, from that coordinate's two control values. */
function coordinate(s: number, first: number, second: number): number {
	const rest = 1 - s;
	return 3 * rest * rest * s * first + 3 * rest * s * s * second + s * s * s;
}

/**
 * The curve the control transitions ease on, as a function of time from 0 to 1. A CSS transition
 * names the curve itself. This is for a transition Svelte runs, which takes its easing as a
 * function.
 */
export function controlEase(time: number): number {
	if (time <= 0) return 0;
	if (time >= 1) return 1;
	// The curve's x rises with its parameter, so halving the range finds the parameter at `time`.
	let [low, high] = [0, 1];
	for (;;) {
		const middle = (low + high) / 2;
		// No number lies between the bounds: `middle` is the parameter to double precision.
		if (middle === low || middle === high) return coordinate(middle, Y1, Y2);
		if (coordinate(middle, X1, X2) < time) low = middle;
		else high = middle;
	}
}
