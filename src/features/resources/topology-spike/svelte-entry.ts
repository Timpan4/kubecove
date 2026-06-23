import { mount } from "svelte";
import TopologyHarness from "./TopologyHarness.svelte";

const target = document.getElementById("root");

if (!target) {
	throw new Error("Missing #root for Svelte topology spike");
}

mount(TopologyHarness, { target });
