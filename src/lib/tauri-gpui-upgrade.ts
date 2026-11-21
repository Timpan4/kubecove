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

async function saveBeforeHandoff(): Promise<void> {
	try {
		await exportTauriMigrationSnapshot();
	} catch {
		console.warn("Could not save the migration snapshot.");
	}
}

export async function installGpuiUpgrade(version: string): Promise<GpuiUpgradeInfo | null> {
	await saveBeforeHandoff();
	return local.invoke("install_gpui_upgrade", { version });
}

export async function launchGpuiUpgrade(): Promise<void> {
	await saveBeforeHandoff();
	return local.invoke("launch_gpui_upgrade");
}

export async function openDesktopReleases(): Promise<void> {
	await saveBeforeHandoff();
	return local.invoke("open_desktop_releases_page");
}

export async function revealGpuiUpgrade(): Promise<void> {
	await saveBeforeHandoff();
	return local.invoke("reveal_gpui_upgrade");
}
