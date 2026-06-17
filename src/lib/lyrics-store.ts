import { create } from "zustand";

interface LyricsStore {
  open: boolean;
  toggle: () => void;
  show: () => void;
  hide: () => void;
}

export const useLyricsStore = create<LyricsStore>((set) => ({
  open: false,
  toggle: () => set((s) => ({ open: !s.open })),
  show: () => set({ open: true }),
  hide: () => set({ open: false }),
}));
