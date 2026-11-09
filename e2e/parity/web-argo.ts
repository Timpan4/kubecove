import type { Locator, Page } from "playwright-core";
import type { Step } from "./contract";

/** The control a dialog reachability check names, as the web renders it. */
function argoDialogControl(dialog: Locator, control: string): Locator {
    switch (control) {
        case "External URL": return dialog.getByRole("radio", { name: "External URL", exact: true });
        case "Token": return dialog.locator("#argo-token");
        case "Remember credential": return dialog.getByRole("switch", { name: "Remember credential in native keyring", exact: true });
        case "Connect": return dialog.getByRole("button", { name: "Connect", exact: true });
        default: throw new Error(`Unknown Argo connection dialog control ${control}`);
    }
}

export async function argoAction(page: Page, step: Step) {
    switch (step.action) {
        case "reconnectArgoProfile":
            await page.getByRole("tabpanel").getByRole("button", { name: /^(Manage connection|Connect Argo CD)$/ }).click();
            await page.getByRole("dialog").getByRole("button", { name: "Reconnect", exact: true }).click();
            break;
        case "expandArgoDiff": await page.getByRole("button", { name: /^Show all .* lines$/ }).first().click(); break;
        case "configureArgoInventory":
            await page.evaluate(`import('/src/lib/tauri-dev-mock-argo.ts').then(m => m.configureArgoInventory(${JSON.stringify(step.mode)}))`);
            await page.getByRole("button", { name: "Live updates and refresh options", exact: true }).first().click();
            await page.getByRole("button", { name: "Refresh (clear cache)", exact: true }).click();
            await page.keyboard.press("Escape");
            break;
        case "copyArgoValue": {
            await page.evaluate(() => Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async (value: string) => { document.documentElement.dataset.argoCopy = value; } } }));
            if (step.control.endsWith("value")) await page.getByRole("button", { name: new RegExp(`^Copy ${step.control}:`) }).click();
            else {
                const suffix = step.control === "source" ? "Activate to copy source details." : "Activate to copy.";
                const card = page.locator("div.group.relative").filter({ has: page.getByRole("button", { name: "Open details for Application platform-cilium in argocd", exact: true }) });
                await card.locator(`[aria-label$="${suffix}"]`).filter({ hasText: step.control === "source" ? "" : step.contains }).first().click();
            }
            if (!(await page.locator("html").getAttribute("data-argo-copy"))?.includes(step.contains)) throw new Error("Wrong Argo clipboard value");
            break;
        }
        case "configureArgoMock":
            // `restartClient` reloads the page; the script then applies to startup reads, as the native fixture's does.
            await page.addInitScript((script) => { (globalThis as { __kubecoveParityArgo?: string }).__kubecoveParityArgo = script; }, step.script);
            await page.evaluate(`import('/src/lib/tauri-dev-mock-argo.ts').then(m => m.configureArgoMock(${JSON.stringify(step.script)}))`);
            break;
        case "controlClock":
            // Pausing holds the tracker's recovery tick until advanceClock runs it. Place this after the GitOps rows render,
            // because paused timers also stop rendering.
            await page.clock.pauseAt(new Date(Date.now() + 1000));
            break;
        case "advanceClock": await page.clock.runFor(step.ms); break;
        case "focusArgoResource":
            await page.getByRole("tabpanel").getByRole("button", { name: step.label, exact: false }).first().click();
            break;
        case "expectNoSavedArgoProfile": {
            await page.getByRole("tabpanel").getByRole("button", { name: /^(Manage connection|Connect Argo CD)$/ }).click();
            await page.getByRole("dialog", { name: "Argo CD connection" }).getByText(step.url, { exact: true }).waitFor({ state: "hidden" });
            await page.keyboard.press("Escape");
            break;
        }
        case "argoControl": {
            if (step.control === "Technical detail") {
                await page.getByText("Technical detail", { exact: true }).first().click();
            } else if (step.control === "Open connection") {
                await page.getByRole("tabpanel").getByRole("button", { name: /^(Manage connection|Connect Argo CD)$/ }).click();
                await page.getByRole("dialog").getByRole("radio", { name: "Found in cluster", exact: true }).click();
            } else if (step.control === "Close connection") {
                await page.keyboard.press("Escape");
            } else if (step.control === "Forget") {
                await page.getByRole("tabpanel").getByRole("button", { name: /^(Manage connection|Connect Argo CD)$/ }).click();
                const dialog = page.getByRole("dialog", { name: "Argo CD connection" });
                await dialog.getByRole("button", { name: "Forget", exact: true }).click();
                await dialog.getByRole("button", { name: "Forget", exact: true }).waitFor({ state: "hidden" });
                await page.keyboard.press("Escape");
            } else await page.getByRole("button", { name: step.control, exact: true }).first().click();
            break;
        }
        case "connectArgoProfile": {
            await (step.surface === "page" ? page.getByRole("button", { name: "Argo CD connection", exact: true }) : page.getByRole("tabpanel").getByRole("button", { name: /^(Manage connection|Connect Argo CD)$/ })).click();
            const dialog = page.getByRole("dialog", { name: "Argo CD connection" });
            if (step.tunnel) {
                await dialog.getByRole("radio", { name: "Found in cluster", exact: true }).click();
                await dialog.getByRole("radio", { name: /argocd\/argocd-server:443/ }).click();
            } else {
                await dialog.getByRole("radio", { name: "External URL", exact: true }).click();
                await dialog.getByLabel("Argo CD server URL", { exact: true }).fill(step.url);
            }
            if (step.local) {
                await dialog.getByRole("radio", { name: "Local login", exact: true }).click();
                await dialog.getByLabel("Username", { exact: true }).fill("fixture-user");
                await dialog.getByLabel("Password", { exact: true }).fill("fixture-password");
            } else await dialog.getByLabel("Token", { exact: true }).fill("parity-fixture-token");
            if (step.insecure || step.customCa) {
                await dialog.getByText("Advanced", { exact: true }).click();
                if (step.customCa) await dialog.getByLabel("Custom CA PEM (session only)", { exact: true }).fill(step.customCa);
                if (step.insecure) {
                    // The dialog keeps the switch between connects, so turn it on only when it is off.
                    const insecure = dialog.getByRole("switch").first();
                    if ((await insecure.getAttribute("aria-checked")) !== "true") await insecure.click();
                    await dialog.getByText("Certificate validation is disabled and never saved.", { exact: true }).waitFor();
                }
            }
            // The dialog keeps the switch between connects, so turn it on only when it is off.
            const remember = dialog.getByRole("switch", { name: "Remember credential in native keyring", exact: true });
            if (step.remember && (await remember.getAttribute("aria-checked")) !== "true") await remember.click();
            await dialog.getByRole("button", { name: "Connect", exact: true }).click();
            if (step.tunnel) { await dialog.getByText("Confirm tunnel target", { exact: true }).waitFor(); break; }
            await dialog.getByRole("button", { name: "Disconnect", exact: true }).waitFor();
            await page.keyboard.press("Escape");
            break;
        }
        case "disconnectArgoProfile": {
            await page.getByRole("tabpanel").getByRole("button", { name: "Manage connection", exact: true }).click();
            const dialog = page.getByRole("dialog", { name: "Argo CD connection" });
            await dialog.getByRole("button", { name: "Disconnect", exact: true }).click();
            await dialog.getByRole("button", { name: "Reconnect", exact: true }).waitFor();
            await page.keyboard.press("Escape");
            break;
        }
        case "reviewArgoOperation":
            if (step.operation === "Hard refresh") {
                await page.getByRole("button", { name: "Refresh options", exact: true }).click();
                await page.getByRole("button", { name: "Hard refresh", exact: true }).click();
            } else await page.getByRole("tabpanel").getByRole("button", { name: step.operation, exact: true }).click();
            break;
        case "confirmArgoReview": await page.getByRole("button", { name: "Confirm Argo CD operation", exact: true }).click(); break;
        case "cancelArgoReview": await page.getByRole("button", { name: "Cancel review", exact: true }).click(); break;
        case "expectArgoReview": {
            const review = page.getByRole("region", { name: "Review Argo CD operation" });
            await review.getByText(`${step.operation} ${step.transport} mock-dev Application ${step.namespace} ${step.name}`, { exact: true }).waitFor();
            break;
        }
        case "expectArgoText": {
            const text = page.getByText(step.text, { exact: false }).first();
            await text.waitFor({ state: "attached" });
            const technical = page.locator("details").filter({ has: text });
            if (await technical.count() && !(await text.isVisible())) await technical.locator("summary").click();
            await text.waitFor();
            break;
        }
        case "argoDocument":
            await page.getByRole("tabpanel").getByRole("radio", { name: step.view, exact: true }).click();
            await page.getByRole("tabpanel").getByText(step.contains, { exact: false }).first().waitFor();
            break;
        case "confirmArgoTunnel": {
            const dialog = page.getByRole("dialog", { name: "Argo CD connection" });
            await dialog.getByText("argocd-server-7d9f8b6c5-x2k4q", { exact: true }).waitFor();
            await dialog.getByText("SHA-256", { exact: false }).waitFor();
            // Another saved profile may already show Disconnect, so wait for one more.
            const disconnects = dialog.getByRole("button", { name: "Disconnect", exact: true });
            const connected = await disconnects.count();
            await dialog.getByRole("button", { name: "Confirm and connect", exact: true }).click();
            if (step.failure) { await dialog.getByText(step.failure, { exact: false }).first().waitFor(); break; }
            await disconnects.nth(connected).waitFor();
            await page.keyboard.press("Escape"); break;
        }
        case "expectArgoPopover": {
            const name = step.control === "Sync options" ? "Sync with these options" : "Hard refresh";
            await page.locator("[data-slot=popover-content]").filter({ has: page.getByRole("button", { name, exact: true }) }).waitFor({ state: step.open ? "visible" : "hidden" });
            break;
        }
        case "dismissArgoPopover":
            if (step.how === "Escape") await page.keyboard.press("Escape");
            else await page.getByText("These operations target the Argo CD Application", { exact: false }).first().click();
            break;
        case "expectArgoCheckbox":
            await page.locator("[data-slot=popover-content]").getByRole("checkbox", { name: step.name, exact: true, checked: step.checked }).waitFor();
            break;
        case "toggleArgoOption":
            await page.locator("[data-slot=popover-content]").getByRole("checkbox", { name: step.name, exact: true }).click();
            break;
        case "expectNoArgoControl":
            await page.getByRole("button", { name: step.control, exact: true }).waitFor({ state: "detached" });
            break;
        case "selectArgoPreference":
            await page.getByRole("combobox", { name: "Argo CD inspection preference", exact: true }).click();
            await page.getByRole("option", { name: step.preference === "Kubernetes" ? "Kubernetes" : "Automatic", exact: false }).click();
            break;
        case "expectSavedArgoProfile":
            await page.getByRole("tabpanel").getByRole("button", { name: /^(Manage connection|Connect Argo CD)$/ }).click();
            await page.getByRole("dialog", { name: "Argo CD connection" }).getByText(step.url, { exact: true }).waitFor();
            await page.keyboard.press("Escape");
            break;
        case "configureArgoSync": {
            await page.getByRole("button", { name: "Sync options", exact: true }).click();
            const options = page.locator("[data-slot=popover-content]");
            await options.getByRole("textbox").fill(step.revision);
            await options.getByRole("checkbox", { name: "Prune", exact: true }).click();
            await options.getByRole("checkbox", { name: "Dry run", exact: true }).click();
            await options.getByRole("checkbox", { name: "Force / replace", exact: true }).click();
            await options.getByRole("button", { name: "Sync with these options", exact: true }).click();
            const confirmation = page.getByRole("dialog", { name: "Confirm advanced sync" });
            await confirmation.getByRole("textbox").fill(step.name);
            await confirmation.getByRole("button", { name: "Confirm sync", exact: true }).click();
            break;
        }
        case "selectArgoHistory":
            await page.getByRole("combobox", { name: "Recent deployment", exact: true }).click();
            await page.getByRole("option", { name: step.revision, exact: true }).click();
            break;
        case "expectArgoWorkspaceTransport": await page.getByText(step.text, { exact: false }).first().waitFor(); break;
        case "completeArgoOperation":
            await page.evaluate("import('/src/lib/tauri-dev-mock-argo.ts').then(m => m.completeMockArgoOperation())");
            break;

        case "selectArgoConnection":
            await page.getByRole("combobox", { name: "Argo CD inspection preference", exact: true }).click();
            await page.getByRole("option", { name: step.url, exact: true }).click();
            break;
        case "expectArgoFocusInPopover":
            // The web moves focus into the popover on open, and Tab must not leave it.
            await page.waitForFunction((label: string) => Array.from(document.querySelectorAll("[data-slot=popover-content]"))
                .some((element) => element.textContent?.includes(label) && element.contains(document.activeElement)),
                step.control === "Sync options" ? "Sync with these options" : "Hard refresh");
            break;
        case "argoConnectionDialog": {
            // The Bits dialog keeps Tab inside itself, closes on Escape and on an outside press.
            const dialog = page.getByRole("dialog", { name: "Argo CD connection" });
            switch (step.operation) {
                case "open":
                    await page.getByRole("button", { name: "Argo CD connection", exact: true }).click();
                    await dialog.waitFor();
                    break;
                case "expectFocusIn":
                    if (!(await dialog.evaluate((node) => node.contains(document.activeElement)))) throw new Error("Focus is outside the Argo CD connection dialog");
                    break;
                case "tabCycle":
                case "shiftTabCycle": {
                    // Presses until focus returns to the element focused now. The dialog's element count bounds the cycle.
                    const limit = await dialog.locator("*").count();
                    const first = await page.evaluateHandle(() => document.activeElement);
                    const key = step.operation === "tabCycle" ? "Tab" : "Shift+Tab";
                    for (let stops = 1; ; stops += 1) {
                        if (stops > limit) throw new Error(`Focus did not return to its first control within ${limit} presses`);
                        await page.keyboard.press(key);
                        if (!(await dialog.evaluate((node) => node.contains(document.activeElement)))) throw new Error(`Focus left the Argo CD connection dialog after ${stops} presses`);
                        if (await page.evaluate((start) => document.activeElement === start, first)) break;
                    }
                    break;
                }
                case "escape":
                    await page.keyboard.press("Escape");
                    await dialog.waitFor({ state: "hidden" });
                    break;
                case "outsideClick":
                    await page.mouse.click(4, 4);
                    await dialog.waitFor({ state: "hidden" });
                    break;
                case "expectConnectForm":
                    // The form's Connect button sits under the saved connections, so the dialog shows it at once.
                    if (!(await dialog.getByRole("button", { name: "Connect", exact: true }).isVisible())) throw new Error("The Argo CD connection form is not shown");
                    break;
                case "expectInWindow": {
                    // The dialog's box must lie inside the window, so no edge is clipped.
                    const box = await dialog.boundingBox();
                    const viewport = page.viewportSize();
                    if (!box || !viewport) throw new Error("The Argo CD connection dialog has no box");
                    if (box.x < 0 || box.y < 0 || box.x + box.width > viewport.width || box.y + box.height > viewport.height) {
                        throw new Error(`The Argo CD connection dialog ${JSON.stringify(box)} overflows the ${viewport.width}x${viewport.height} window`);
                    }
                    break;
                }
                case "expectReachable": {
                    // Scrolling the dialog brings the control in, and the control then lies inside the dialog's box.
                    const control = argoDialogControl(dialog, step.control);
                    await control.scrollIntoViewIfNeeded();
                    const box = await control.boundingBox();
                    const outer = await dialog.boundingBox();
                    if (!box || !outer) throw new Error(`${step.control} has no box in the Argo CD connection dialog`);
                    if (box.x < outer.x || box.y < outer.y || box.x + box.width > outer.x + outer.width || box.y + box.height > outer.y + outer.height) {
                        throw new Error(`${step.control} is not reachable inside the Argo CD connection dialog`);
                    }
                    break;
                }
                case "expectOpen":
                    await dialog.waitFor({ state: step.open ? "visible" : "hidden" });
                    break;
                case "expectRemember": {
                    const checked = await dialog.getByRole("switch", { name: "Remember credential in native keyring", exact: true }).getAttribute("aria-checked");
                    if ((checked === "true") !== step.checked) throw new Error(`Remember credential switch is ${checked}`);
                    break;
                }
                case "expectLocalLogin": {
                    const checked = await dialog.getByRole("radio", { name: "Local login", exact: true }).getAttribute("aria-checked");
                    if ((checked === "true") !== step.checked) throw new Error(`Local login choice is ${checked}`);
                    break;
                }
                case "chooseLocalLogin":
                    await dialog.getByRole("radio", { name: "Local login", exact: true }).click();
                    break;
                // Opens the detail's dialog without choosing a server source, unlike "Open connection".
                case "openDetail":
                    await page.getByRole("tabpanel").getByRole("button", { name: /^(Manage connection|Connect Argo CD)$/ }).click();
                    await dialog.waitFor();
                    break;
                case "chooseFoundInCluster":
                case "chooseExternalUrl":
                    await dialog.getByRole("radio", { name: step.operation === "chooseExternalUrl" ? "External URL" : "Found in cluster", exact: true }).click();
                    break;
                case "expectFoundInCluster": {
                    const checked = await dialog.getByRole("radio", { name: "Found in cluster", exact: true }).getAttribute("aria-checked");
                    if ((checked === "true") !== step.checked) throw new Error(`Found in cluster choice is ${checked}`);
                    break;
                }
                case "expectInsecure":
                case "chooseInsecure": {
                    // The invalid-certificate switch sits under the external URL's Advanced disclosure.
                    await dialog.getByRole("radio", { name: "External URL", exact: true }).click();
                    const label = dialog.getByText("Accept invalid TLS certificate for this session", { exact: true });
                    if (!(await label.isVisible())) await dialog.getByText("Advanced", { exact: true }).click();
                    const insecure = dialog.getByRole("switch").first();
                    const checked = await insecure.getAttribute("aria-checked");
                    if (step.operation === "chooseInsecure") {
                        if (checked !== "true") await insecure.click();
                    } else if ((checked === "true") !== step.checked) throw new Error(`Invalid certificate switch is ${checked}`);
                    break;
                }
            }
            break;
        }
        case "tabArgoPopover":
            for (let press = 0; press < step.presses; press += 1) await page.keyboard.press("Tab");
            break;
        case "expectArgoOptionFocusable": {
            const option = page.locator("[data-slot=popover-content]").getByRole("checkbox", { name: step.name, exact: true });
            await option.waitFor();
            if (!(await option.evaluate((node) => node.tabIndex >= 0 && !(node as HTMLButtonElement).disabled))) throw new Error(`${step.name} is not keyboard focusable`);
            break;
        }
        case "expectArgoTriggerFocus": {
            const trigger = page.getByRole("button", { name: step.control, exact: true });
            await trigger.waitFor();
            if (!(await trigger.evaluate((node) => node === document.activeElement))) throw new Error(`${step.control} trigger is not focused`);
            break;
        }
        case "expectArgoOperationState": {
            const locator = step.control === "Sync options" || step.control === "Refresh options"
                ? page.getByRole("button", { name: step.control, exact: true })
                : page.getByRole("tabpanel").getByRole("button", { name: step.control, exact: true });
            await locator.waitFor();
            if ((await locator.isDisabled()) === step.enabled) throw new Error(`${step.control} enabled state is not ${step.enabled}`);
            break;
        }

        case "fillArgoConnectionField":
        case "expectArgoConnectionField": {
            const dialog = page.getByRole("dialog", { name: "Argo CD connection" });
            const labels = { url: "Argo CD server URL", rootPath: "Root path", tlsServerName: "TLS server name", token: "Token", password: "Password" };
            const field = dialog.getByLabel(labels[step.field], { exact: true });
            // The root path and TLS server name sit under the discovered server's Advanced disclosure.
            if ((step.field === "rootPath" || step.field === "tlsServerName") && !(await field.isVisible())) await dialog.getByText("Advanced", { exact: true }).click();
            if (step.action === "fillArgoConnectionField") await field.fill(step.value);
            // The error omits the value, which can be a credential.
            else if ((await field.inputValue()) !== step.value) throw new Error(`${step.field} does not hold the expected value`);
            break;
        }
        default: throw new Error(`Unknown Argo action ${step.action}`);
    }
}
