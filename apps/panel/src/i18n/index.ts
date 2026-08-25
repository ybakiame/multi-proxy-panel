import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import zhCN from "./zh-CN.json";
import enUS from "./en-US.json";

const resources = {
  "zh-CN": { translation: zhCN },
  "en-US": { translation: enUS },
};

function getInitialLanguage(): "zh-CN" | "en-US" {
  try {
    const raw = localStorage.getItem("pp-settings");
    if (raw) {
      const parsed = JSON.parse(raw);
      if (parsed.state?.language === "en-US" || parsed.state?.language === "zh-CN") {
        return parsed.state.language;
      }
    }
  } catch {
    // ignore
  }
  return "zh-CN";
}

i18n.use(initReactI18next).init({
  resources,
  lng: getInitialLanguage(),
  fallbackLng: "zh-CN",
  interpolation: {
    escapeValue: false,
  },
});

export default i18n;
