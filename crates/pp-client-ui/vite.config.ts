import path from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react-swc";
import { defineConfig } from "vite";

// Tauri 桌面壳（src-tauri/tauri.conf.json devUrl = http://localhost:1420）。
// 生产构建产物输出到 dist/，由 Tauri 的 frontendDist 指向。
export default defineConfig({
  plugins: [react(), tailwindcss()],
  base: "./",
  build: {
    outDir: "dist",
    sourcemap: true,
    chunkSizeWarningLimit: 1200,
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    clearScreen: false,
  },
});
