import { describe, expect, test } from "bun:test";
import { tauriEnvironment } from "../scripts/tauri";

const webviewArguments = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS";
const cargoTargetDirectory = "CARGO_TARGET_DIR";

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
		const localAppData = { LOCALAPPDATA: "C:\\Users\\operator\\AppData\\Local" };
		const buildEnvironment = tauriEnvironment(["build"], localAppData, "win32");
		const linuxEnvironment = tauriEnvironment(["dev"], localAppData, "linux");

		expect(buildEnvironment[webviewArguments]).toBeUndefined();
		expect(buildEnvironment[cargoTargetDirectory]).toBeUndefined();
		expect(linuxEnvironment[webviewArguments]).toBeUndefined();
		expect(linuxEnvironment[cargoTargetDirectory]).toBeUndefined();
	});

	test("keeps Windows dev incremental state off the workspace drive", () => {
		const environment = tauriEnvironment(
			["dev"],
			{ LOCALAPPDATA: "C:\\Users\\operator\\AppData\\Local" },
			"win32",
		);

		expect(environment[cargoTargetDirectory]).toBe(
			"C:\\Users\\operator\\AppData\\Local\\KubeCove\\cargo-target",
		);
	});

	test("keeps an explicitly configured Cargo target directory", () => {
		const environment = tauriEnvironment(
			["dev"],
			{
				LOCALAPPDATA: "C:\\Users\\operator\\AppData\\Local",
				[cargoTargetDirectory]: "E:\\cargo-target",
			},
			"win32",
		);

		expect(environment[cargoTargetDirectory]).toBe("E:\\cargo-target");
	});
});
