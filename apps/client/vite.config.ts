import path from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

/**
 * 客户端单包双入口（apps/client，见 docs/plans/2026-10-03-client-merge-evaluation.md）：
 * vite mode 构建期平台分发，平台是构建期事实而非运行时判断。
 *
 * - desktop（默认 mode）：桌面 UI（`src/desktop`，HeroUI + react-compiler），
 *   dev 端口 1420（Tauri 桌面基线 `src-tauri/tauri.conf.json` devUrl 一致）；
 * - android（`--mode android`）：移动 UI（`src/mobile`，Konsta，不开 react-compiler），
 *   dev 端口 1430（`src-tauri/tauri.android.conf.json` overlay devUrl 一致）。
 *
 * `@app` alias 指向当前 mode 的平台目录，产物互不含对方 UI 库（M0-③ 实证）。
 */
export default defineConfig(({ mode }) => {
  const isAndroid = mode === "android";
  const port = isAndroid ? 1430 : 1420;
  return {
    // prevent vite from obscuring rust errors
    clearScreen: false,
    plugins: [react(isAndroid ? {} : { compiler: true }), tailwindcss()],
    // ADR-0013 D2：编译期平台事实注入 @pp/ui（IS_MOBILE 分支经 DCE 裁剪死代码）
    define: {
      __PP_PLATFORM__: JSON.stringify(isAndroid ? "mobile" : "desktop"),
    },
    base: "./",
    resolve: {
      alias: {
        "@app": path.resolve(import.meta.dirname, isAndroid ? "./src/mobile" : "./src/desktop"),
      },
    },
    server: {
      // make sure this port matches the devUrl port in the corresponding tauri conf
      port,
      // Tauri expects a fixed port, fail if that port is not available
      strictPort: true,
      // if the host Tauri is expecting is set, use it
      host: host || false,
      hmr: host
        ? {
            protocol: "ws",
            host,
            port: port + 1,
          }
        : undefined,
      watch: {
        // tell vite to ignore watching `src-tauri`
        ignored: ["**/src-tauri/**"],
      },
    },
    // Env variables starting with the item of `envPrefix` will be exposed in tauri's source code through `import.meta.env`.
    envPrefix: ["VITE_", "TAURI_ENV_*"],
    build: {
      outDir: "dist",
      // Tauri uses Chromium on Windows and WebKit on macOS and Linux
      target: process.env.TAURI_ENV_PLATFORM == "windows" ? "chrome105" : "safari13",
      // don't minify for debug builds
      minify: !process.env.TAURI_ENV_DEBUG ? "oxc" : false,
      // produce sourcemaps for debug builds
      sourcemap: !!process.env.TAURI_ENV_DEBUG,
      chunkSizeWarningLimit: 1200,
    },
  };
});
