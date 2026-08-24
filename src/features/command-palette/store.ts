import { createStore } from "zustand/vanilla";

interface CommandPaletteState {
	open: boolean;
	setOpen: (open: boolean) => void;
	toggle: () => void;
}

export const useCommandPaletteStore = createStore<CommandPaletteState>((set) => ({
	open: false,
	setOpen: (open) => set({ open }),
	toggle: () => set((state) => ({ open: !state.open })),
}));
