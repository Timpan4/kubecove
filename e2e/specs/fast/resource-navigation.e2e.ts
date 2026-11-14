import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { describe, it } from "mocha";

// Navigation must not depend on the height of the overview's loading content.
// Release its real mock resource read between pointerdown and click.
describe("resource navigation while overview data arrives", () => {
  it("opens resources while the overview grows during navigation", async () => {
    await browser.url("/");
    await browser.execute(() => { localStorage.clear(); sessionStorage.clear(); });
    await browser.refresh();
    await $("#workspace-name").setValue("Navigation race");
    await browser.execute(async () => {
      const module = "/src/lib/tauri-dev-mock-timing.ts";
      const { mockTiming } = await import(/* @vite-ignore */ module);
      mockTiming.control({ operation: "hold", command: "list_resource_scope", context: "mock-dev" });
    });
    const create = await $("button=Create workspace");
    await create.waitForEnabled();
    await create.click();
    await browser.waitUntil(() => browser.execute(async () => {
      const module = "/src/lib/tauri-dev-mock-timing.ts";
      return (await import(/* @vite-ignore */ module)).mockTiming.pending("list_resource_scope", "mock-dev");
    }));
    await browser.execute(async () => {
      const module = "/src/lib/tauri-dev-mock-timing.ts";
      const { mockTiming } = await import(/* @vite-ignore */ module);
      document.addEventListener("pointerdown", () => {
        mockTiming.control({ operation: "release", command: "list_resource_scope", context: "mock-dev" });
      }, { capture: true, once: true });
    });
    await $("button=Open resources").click();
    await expect($('input[aria-label="Search resources"]')).toBeDisplayed();
    const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
    if (artifacts) await browser.saveScreenshot(join(artifacts, "resource-navigation.png"));
  });
});
