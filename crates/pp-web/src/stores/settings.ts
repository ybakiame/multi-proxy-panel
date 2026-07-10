import { create } from "zustand";
import { persist } from "zustand/middleware";

export type Theme = "dark" | "light";
export type Language = "zh-CN" | "en-US";

interface SettingsState {
  language: Language;
  theme: Theme;
  setLanguage: (language: Language) => void;
  setTheme: (theme: Theme) => void;
  toggleTheme: () => void;
}

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set) => ({
      language: "zh-CN",
      theme: "dark",
      setLanguage: (language) => set({ language }),
      setTheme: (theme) => set({ theme }),
      toggleTheme: () =>
        set((state) => ({ theme: state.theme === "dark" ? "light" : "dark" })),
    }),
    {
      name: "pp-settings",
      partialize: (state) => ({ language: state.language, theme: state.theme }),
    },
  ),
);
