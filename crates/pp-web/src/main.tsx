import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { HelmetProvider } from "react-helmet-async";
import { Provider as JotaiProvider } from "jotai";
import { AuthProvider } from "./context/AuthContext";
import { SettingsProvider } from "./components/SettingsProvider";
import App from "./App";
import "./i18n";
import "./index.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <HelmetProvider>
      <JotaiProvider>
        <BrowserRouter>
          <AuthProvider>
            <SettingsProvider>
              <App />
            </SettingsProvider>
          </AuthProvider>
        </BrowserRouter>
      </JotaiProvider>
    </HelmetProvider>
  </StrictMode>,
);
