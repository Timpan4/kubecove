import { useEffect } from "react";
import { createTauriClient, detectArgoCD } from "@/lib/tauri";
import { diagnosticLog } from "@/lib/diagnostics";

export function useArgoDetection(
	clusterContext: string,
	setArgoDetected: (detected: boolean) => void,
) {
	useEffect(() => {
		if (!clusterContext) {
			setArgoDetected(false);
			return;
		}
		let cancelled = false;
		const client = createTauriClient();
		detectArgoCD(client, clusterContext)
			.then((detected) => {
				if (!cancelled) {
					diagnosticLog("app.argo.detect.done", {
						cluster: clusterContext,
						detected,
					});
					setArgoDetected(detected);
				}
			})
			.catch(() => {
				if (!cancelled) {
					diagnosticLog("app.argo.detect.error", { cluster: clusterContext });
					setArgoDetected(false);
				}
			});
		return () => {
			cancelled = true;
		};
	}, [clusterContext, setArgoDetected]);
}
