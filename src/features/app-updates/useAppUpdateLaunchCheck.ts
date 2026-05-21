import { useEffect, useRef } from "react";

import { isAppUpdatesEnabled } from "@/lib/release-channel";
import { useAppUpdateStore } from "./store";

export function useAppUpdateLaunchCheck() {
	const checkedRef = useRef(false);
	const checkForUpdates = useAppUpdateStore((state) => state.checkForUpdates);

	useEffect(() => {
		if (!isAppUpdatesEnabled()) return;
		if (checkedRef.current) return;
		checkedRef.current = true;
		void checkForUpdates({ manual: false });
	}, [checkForUpdates]);
}
