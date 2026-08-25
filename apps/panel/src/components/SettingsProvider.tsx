import { useEffect } from "react";
import { useAtom } from "jotai";
import { useTranslation } from "react-i18next";
import { useSettingsStore } from "../stores/settings";
import { languageAtom, themeAtom } from "../atoms/settings";

export function SettingsProvider({ children }: { children: React.ReactNode }) {
  const { i18n } = useTranslation();
  const [_language, setLanguage] = useAtom(languageAtom);
  const [_theme, setTheme] = useAtom(themeAtom);

  useEffect(() => {
    const unsubscribe = useSettingsStore.subscribe((state, prevState) => {
      if (state.language !== prevState.language) {
        setLanguage(state.language);
        i18n.changeLanguage(state.language);
        document.documentElement.lang = state.language;
      }
      if (state.theme !== prevState.theme) {
        setTheme(state.theme);
        document.documentElement.classList.remove("dark", "light");
        document.documentElement.classList.add(state.theme);
        document.documentElement.setAttribute("data-theme", state.theme);
      }
    });

    // Apply initial settings once on mount.
    const initial = useSettingsStore.getState();
    if (i18n.language !== initial.language) {
      i18n.changeLanguage(initial.language);
    }
    document.documentElement.lang = initial.language;
    document.documentElement.classList.remove("dark", "light");
    document.documentElement.classList.add(initial.theme);
    document.documentElement.setAttribute("data-theme", initial.theme);
    setLanguage(initial.language);
    setTheme(initial.theme);

    return unsubscribe;
  }, [i18n, setLanguage, setTheme]);

  return <>{children}</>;
}
