import { describe, expect, test } from "bun:test";
import { tauriEnvironment } from "../scripts/tauri";

const webviewArguments = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS";

describe("Tauri development launcher", () => {
	test("enables a localhost WebView2 debug endpoint for Windows development", () => {
		const environment = tauriEnvironment(["dev"], {}, "win32");

		expect(environment[webviewArguments]).toBe(
			"--remote-debugging-port=9222 --remote-debugging-address=127.0.0.1 --remote-allow-origins=http://127.0.0.1:9222,http://localhost:9222",
		);
	});

	test("keeps existing WebView2 arguments and supports a custom port", () => {
		const environment = tauriEnvironment(
			["dev"],
			{
				KUBECOVE_DEVTOOLS_PORT: "9333",
				[webviewArguments]: "--disable-features=ExampleFeature",
			},
			"win32",
		);

		expect(environment[webviewArguments]).toStartWith(
			"--disable-features=ExampleFeature ",
		);
		expect(environment[webviewArguments]).toContain(
			"--remote-debugging-port=9333",
		);
	});

	test("does not replace an explicitly configured debug port", () => {
		const existing = "--remote-debugging-port=9444 --disable-gpu";
		const environment = tauriEnvironment(
			["dev"],
			{ [webviewArguments]: existing },
			"win32",
		);

		expect(environment[webviewArguments]).toBe(existing);
	});

	test("does not enable WebView2 debugging for builds or other platforms", () => {
		expect(
			tauriEnvironment(["build"], {}, "win32")[webviewArguments],
		).toBeUndefined();
		expect(
			tauriEnvironment(["dev"], {}, "linux")[webviewArguments],
		).toBeUndefined();
	});
});
