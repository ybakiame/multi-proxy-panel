import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [react({ compiler: true }), tailwindcss()],
  base: "/",
  build: {
    outDir: "dist",
    sourcemap: true,
    chunkSizeWarningLimit: 1200,
    rollupOptions: {
      output: {
        manualChunks(id: string) {
          if (id.includes("@heroui/react")) {
            return "heroui";
          }
          if (
            id.includes("react") ||
            id.includes("react-dom") ||
            id.includes("react-router-dom") ||
            id.includes("axios") ||
            id.includes("i18next") ||
            id.includes("react-i18next")
          ) {
            return "vendor";
          }
        },
      },
    },
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    port: 8080,
    proxy: {
      "/api": {
        target: "http://localhost:8081",
        changeOrigin: true,
      },
    },
  },
});
