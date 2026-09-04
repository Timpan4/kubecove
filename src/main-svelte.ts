import "./App.css";
import { markStartup } from "./lib/startup-marks";
import { mount } from "svelte";
import Root from "./app/svelte/Root.svelte";

document.documentElement.classList.add("dark");

const target = document.getElementById("root");
if (!target) {
	throw new Error("Missing #root for Svelte app shell");
}

mount(Root, { target });

markStartup("svelte-mount");
