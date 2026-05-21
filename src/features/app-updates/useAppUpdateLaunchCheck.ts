import { useEffect, useRef } from "react";

import { useAppUpdateStore } from "./store";

export function useAppUpdateLaunchCheck() {
	const checkedRef = useRef(false);
	const checkForUpdates = useAppUpdateStore((state) => state.checkForUpdates);

	useEffect(() => {
		if (checkedRef.current) return;
		checkedRef.current = true;
		void checkForUpdates({ manual: false });
	}, [checkForUpdates]);
}
