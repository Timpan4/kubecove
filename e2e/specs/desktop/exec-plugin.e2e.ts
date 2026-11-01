import { readFile, realpath, writeFile } from "node:fs/promises";
import { createServer, type AddressInfo, type Socket, type Server } from "node:net";
import { resolve, sep } from "node:path";
import { browser, expect } from "@wdio/globals";
import { describe, it } from "mocha";
import { parseDocument } from "yaml";
import type { AppError, ClusterContext } from "../../../src/lib/types";

function isTcpAddress(address: ReturnType<Server["address"]>): address is AddressInfo {
	return address !== null && Object(address) === address;
}

declare global {
	interface Window {
		__KUBECOVE_EXEC_RESULT__?: string;
	}
}

describe("native exec credential plugin", () => {
	it("keeps IPC responsive and cancels a read while its plugin is pending", async () => {
		const configured = process.env.KUBECOVE_KUBECONFIG;
		if (process.env.KUBECOVE_E2E_SMOKE !== "1" || !configured) throw new Error("Exec fixture requires the isolated desktop smoke runner");
		const kubeconfig = await realpath(configured);
		const fixtureRoot = (await realpath(resolve(".e2e", "desktop-smoke"))) + sep;
		if (!kubeconfig.startsWith(fixtureRoot)) throw new Error("Refusing to edit a kubeconfig outside the owned smoke fixture");
		const original = await readFile(kubeconfig, "utf8");
		const sockets = new Set<Socket>();
		let runs = 0;
		let released = false;
		const server = createServer((socket) => {
			runs++;
			sockets.add(socket);
			socket.on("close", () => sockets.delete(socket));
			if (released) socket.write("continue");
		});
		await new Promise<void>((done, reject) => {
			server.once("error", reject);
			server.listen(0, "127.0.0.1", done);
		});
		const address = server.address();
		if (!isTcpAddress(address)) throw new Error("Missing fixture port");
		try {
			const document = parseDocument(original);
			const contexts = await browser.execute(() => window.__TAURI__.core.invoke<ClusterContext[]>("list_kube_contexts"));
			const context = contexts[0]?.name;
			if (!context) throw new Error("Missing smoke context");
			document.setIn(["users", 0, "user"], { exec: {
				apiVersion: "client.authentication.k8s.io/v1",
				command: process.execPath,
				args: [resolve("e2e/fixtures/exec-credential-plugin.mjs"), String(address.port)],
				interactiveMode: "Never",
			} });
			await writeFile(kubeconfig, document.toString());
			await browser.execute((clusterContext) => {
				window.__KUBECOVE_EXEC_RESULT__ = "pending";
				void window.__TAURI__.core.invoke("list_namespaces", { clusterContext }).then(
					() => { window.__KUBECOVE_EXEC_RESULT__ = "unexpected success"; },
					(error: AppError) => {
						window.__KUBECOVE_EXEC_RESULT__ = error.kind;
					},
				);
			}, context);
			await browser.waitUntil(() => runs > 0, { timeoutMsg: "Native command never started the configured plugin" });
			const duringLogin = await browser.execute(() => window.__TAURI__.core.invoke<ClusterContext[]>("list_kube_contexts"));
			expect(duringLogin).toHaveLength(2);
			expect(await browser.execute(() => window.__KUBECOVE_EXEC_RESULT__)).toBe("pending");
			await browser.execute(() => window.__TAURI__.core.invoke("cancel_workspace_requests"));
			await browser.waitUntil(async () => (await browser.execute(() => window.__KUBECOVE_EXEC_RESULT__)) === "cancelled", { timeoutMsg: "Read did not cancel before plugin completion" });
			expect(runs).toBe(1);
		} finally {
			// Finish every dependency-owned subprocess before the runner closes the app.
			released = true;
			for (const socket of sockets) socket.write("continue");
			await writeFile(kubeconfig, original);
			// A replacement read waits for the cancelled build's gate to be released.
			const contexts = await browser.execute(() => window.__TAURI__.core.invoke<ClusterContext[]>("list_kube_contexts"));
			await browser.execute(async (clusterContext) => {
				await window.__TAURI__.core.invoke("list_namespaces", { clusterContext }).catch(() => {});
			}, contexts[0]?.name);
			await new Promise<void>((done) => server.close(() => done()));
		}
	});
});
