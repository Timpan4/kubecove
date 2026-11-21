import { exportTauriMigrationSnapshot } from "./tauri-migration";
import { createNativeTauriClient } from "./tauri-runtime";

type InstallKind = "appImage" | "appBundle" | "windowsInstaller";

export type GpuiUpgradeInfo =
	| { status: "consolidated"; version: string }
	| { status: "unsupported"; target: string; message: string }
	| { status: "unavailable"; target: string }
	| {
			status: "available";
			target: string;
			version: string;
			notes: string | null;
			publishedAt: string | null;
			assetName: string;
			installKind: InstallKind;
	  }
	| { status: "installed"; path: string; version: string; installKind: InstallKind; canLaunch: boolean };

// A selected remote server cannot install or launch software on the desktop.
const local = createNativeTauriClient();

export function checkGpuiUpgrade(): Promise<GpuiUpgradeInfo> {
	return local.invoke("check_gpui_upgrade");
}

export function installGpuiUpgrade(version: string): Promise<GpuiUpgradeInfo | null> {
	return local.invoke("install_gpui_upgrade", { version });
}

export async function launchGpuiUpgrade(): Promise<void> {
	await exportTauriMigrationSnapshot();
	return local.invoke("launch_gpui_upgrade");
}

export async function openDesktopReleases(): Promise<void> {
	await exportTauriMigrationSnapshot();
	const { openUrl } = await import("@tauri-apps/plugin-opener");
	await openUrl("https://github.com/Timpan4/kubecove/releases");
}

export async function revealGpuiUpgrade(): Promise<void> {
	await exportTauriMigrationSnapshot();
	return local.invoke("reveal_gpui_upgrade");
}
