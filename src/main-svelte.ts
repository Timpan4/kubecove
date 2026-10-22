import "./App.css";
import { markStartup } from "./lib/startup-marks";
import { mount } from "svelte";
import Root from "./app/svelte/Root.svelte";
import { IS_DEV_BUILD } from "./lib/build-env";
import { initializeBrowserDevMocks } from "./lib/tauri";

document.documentElement.classList.add("dark");

const target = document.getElementById("root");
if (!target) {
	throw new Error("Missing #root for Svelte app shell");
}

function mountApp(target: HTMLElement): void {
	mount(Root, { target });
	markStartup("svelte-mount");
}

if (IS_DEV_BUILD) {
	void initializeBrowserDevMocks()
		.then(() => mountApp(target))
		.catch((error) => {
			console.error("Failed to start KubeCove", error);
			target.textContent = "KubeCove could not start. Reload the window to try again.";
		});
} else {
	mountApp(target);
}
