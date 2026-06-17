import { create } from "zustand";
import { getTheme, setTheme as setThemeBackend, type Theme } from "@/lib/ipc";

interface ThemeStore {
  theme: Theme;
  loaded: boolean;
  init: () => Promise<void>;
  setTheme: (theme: Theme) => Promise<void>;
}

function systemPrefersDark(): boolean {
  return (
    typeof window !== "undefined" &&
    window.matchMedia?.("(prefers-color-scheme: dark)").matches
  );
}

export function applyTheme(theme: Theme) {
  const isDark = theme === "dark" || (theme === "system" && systemPrefersDark());
  const root = document.documentElement;
  if (isDark) {
    root.classList.add("dark");
  } else {
    root.classList.remove("dark");
  }
}

export const useThemeStore = create<ThemeStore>((set, get) => ({
  theme: "dark",
  loaded: false,
  init: async () => {
    try {
      const theme = await getTheme();
      applyTheme(theme);
      set({ theme, loaded: true });
    } catch {
      applyTheme("dark");
      set({ loaded: true });
    }
    // React to system theme changes when set to "system".
    if (typeof window !== "undefined" && window.matchMedia) {
      const mql = window.matchMedia("(prefers-color-scheme: dark)");
      mql.addEventListener("change", () => {
        if (get().theme === "system") applyTheme("system");
      });
    }
  },
  setTheme: async (theme) => {
    applyTheme(theme);
    set({ theme });
    try {
      await setThemeBackend(theme);
    } catch (e) {
      console.error(e);
    }
  },
}));
