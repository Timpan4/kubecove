interface ActivityCanvas {
	canvas: HTMLCanvasElement;
	context: CanvasRenderingContext2D;
}

const canvases = new Set<ActivityCanvas>();
let frame: number | undefined;
let motion: MediaQueryList | undefined;

function paint({ canvas, context }: ActivityCanvas, time: number) {
	const size = canvas.width;
	const angle = (time / 1150) * Math.PI * 2;
	context.clearRect(0, 0, size, size);
	context.strokeStyle = getComputedStyle(canvas).color;
	context.lineWidth = size / 8;
	context.lineCap = "round";
	context.beginPath();
	context.arc(size / 2, size / 2, size * 0.35, angle, angle + Math.PI * 1.5);
	context.stroke();
}

function animate(time: number) {
	frame = undefined;
	for (const entry of canvases) paint(entry, time);
	if (canvases.size && !motion?.matches && !document.hidden) frame = requestAnimationFrame(animate);
}

function updateMotion() {
	if (frame !== undefined) cancelAnimationFrame(frame);
	frame = undefined;
	if (motion?.matches || document.hidden) {
		for (const entry of canvases) paint(entry, 0);
	} else if (canvases.size) frame = requestAnimationFrame(animate);
}

/** Paint the arc without a CSS animation or composited transform inside scaled WebKit nodes. */
export function drawResourceActivity(canvas: HTMLCanvasElement, active: boolean): () => void {
	const context = canvas.getContext("2d");
	if (!context) return () => {};
	canvas.width = canvas.height = 12 * window.devicePixelRatio;
	const entry = { canvas, context };
	paint(entry, 0);
	if (!active) return () => {};
	if (canvases.size === 0) {
		motion = window.matchMedia("(prefers-reduced-motion: reduce)");
		motion.addEventListener("change", updateMotion);
		document.addEventListener("visibilitychange", updateMotion);
	}
	canvases.add(entry);
	if (frame === undefined) updateMotion();
	return () => {
		canvases.delete(entry);
		if (canvases.size === 0) {
			if (frame !== undefined) cancelAnimationFrame(frame);
			frame = undefined;
			motion?.removeEventListener("change", updateMotion);
			document.removeEventListener("visibilitychange", updateMotion);
			motion = undefined;
		}
	};
}

export const RESOURCE_ACTIVITY_CURRENT = Symbol("resource activity freshness");
