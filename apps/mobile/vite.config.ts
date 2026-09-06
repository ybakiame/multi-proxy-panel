import path from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from '@vitejs/plugin-react'
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

// Tauri 移动壳（src-tauri 于 M3.2 引入，devUrl = http://localhost:1430）。
// 生产构建产物输出到 dist/，由 Tauri 的 frontendDist 指向。
// 说明：未引入 oxc-transform-react（最小依赖集），故 react 插件不开启 compiler 选项。
export default defineConfig({
  // prevent vite from obscuring rust errors
  clearScreen: false,
  plugins: [react(), tailwindcss()],
  base: "./",
  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "./src"),
    },
  },
  server: {
    // make sure this port matches the devUrl port in tauri.conf.json file
    port: 1430,
    // Tauri expects a fixed port, fail if that port is not available
    strictPort: true,
    // if the host Tauri is expecting is set, use it
    host: host || false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1431,
        }
      : undefined,

    watch: {
      // tell vite to ignore watching `src-tauri`
      ignored: ['**/src-tauri/**'],
    },
  },
  // Env variables starting with the item of `envPrefix` will be exposed in tauri's source code through `import.meta.env`.
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    outDir: "dist",
    // Tauri uses Chromium on Windows and WebKit on macOS and Linux
    target:
      process.env.TAURI_ENV_PLATFORM == 'windows'
        ? 'chrome105'
        : 'safari13',
    // don't minify for debug builds
    minify: !process.env.TAURI_ENV_DEBUG ? "oxc" : false,
    // produce sourcemaps for debug builds
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    chunkSizeWarningLimit: 1200,
  },
});
