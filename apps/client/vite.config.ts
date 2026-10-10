import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

/** mode 只决定平台常量、react-compiler 与 dev 端口；页面树共享（ADR-0013）。 */
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
