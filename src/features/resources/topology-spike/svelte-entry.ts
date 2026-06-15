import SvelteTopologyHarness from "./SvelteTopologyHarness.svelte";

const target = document.getElementById("root");

if (!target) {
	throw new Error("Missing #root for Svelte topology spike");
}

new SvelteTopologyHarness({ target });
