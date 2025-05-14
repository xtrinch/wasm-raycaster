// packages/app/vite.config.ts
import wasm from "vite-plugin-wasm";

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react-swc";
import { defineConfig } from "vite";
import crossOriginIsolation from "vite-plugin-cross-origin-isolation";

// https://vite.dev/config/
export default defineConfig({
  assetsInclude: ["**/*.wasm"],
  plugins: [react(), tailwindcss(), wasm(), crossOriginIsolation()],
  base: "wasm-raycaster",
  build: {
    target: "esnext", //browsers can handle the latest ES features
    minify: false,
  },
  worker: {
    format: "es",
  },
});
