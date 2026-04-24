// @ts-nocheck
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";

// Expose the package.json version to the client as import.meta.env.VITE_APP_VERSION
// so the UI can show which release is installed without hardcoding.
const pkg = JSON.parse(
  readFileSync(fileURLToPath(new URL("./package.json", import.meta.url)), "utf8"),
);
process.env.VITE_APP_VERSION = pkg.version;

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

const SVELTE_VIRTUAL = /\.svelte\?/;

function skipSvelteVirtualCss(plugin) {
  if (!plugin?.transform) return plugin;
  const orig = plugin.transform;
  const isObject = typeof orig === "object" && orig !== null;
  const handler = isObject ? orig.handler : orig;
  function wrapped(code, id, opts) {
    if (SVELTE_VIRTUAL.test(id)) return null;
    return handler.call(this, code, id, opts);
  }
  return {
    ...plugin,
    transform: isObject ? { ...orig, handler: wrapped } : wrapped,
  };
}

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [
    sveltekit(),
    // Tailwind v4.2.x's vite plugin tries to parse Svelte virtual style
    // modules (?svelte&...lang.css) as standalone CSS — but they sometimes
    // contain JS (HMR injection, raw .svelte content) and crash with
    // "Invalid declaration". We wrap Tailwind to skip those IDs entirely.
    ...tailwindcss().map((p) => skipSvelteVirtualCss(p)),
  ],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
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
