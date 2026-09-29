import { defineConfig } from "vite";
import preact from "@preact/preset-vite";
import { readFileSync } from "node:fs";

const appVersion = JSON.parse(readFileSync("package.json", "utf8")).version as string;

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [preact()],
  define: { __APP_VERSION__: JSON.stringify(appVersion) },
  // JASSUB (libass renderer, see src/assRenderer.ts) runs in a module
  // worker; its worker/wasm are imported as explicit asset URLs.
  worker: { format: "es" as const },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    // An explicit IPv4 address, matching tauri.conf.json's devUrl: plain
    // "localhost" made Vite listen on IPv6 (::1) only, and the webview's
    // IPv4 attempts took ~2s each to fail - the page arrived up to ~34s
    // late, so the window sat gray without even the splash.
    host: host || "127.0.0.1",
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
