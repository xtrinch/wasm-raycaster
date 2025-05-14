// packages/app/vite.config.ts
import wasm from "vite-plugin-wasm";

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react-swc";
import { defineConfig } from "vite";
import crossOriginIsolation from "vite-plugin-cross-origin-isolation";
import { viteStaticCopy } from "vite-plugin-static-copy";

// https://vite.dev/config/
export default defineConfig({
  assetsInclude: ["**/*.wasm"],
  plugins: [
    react(),
    tailwindcss(),
    wasm(),
    crossOriginIsolation(),
    viteStaticCopy({
      targets: [
        {
          src: "test.js", // wherever your JS file is
          dest: "", // copy directly into dist/
        },
      ],
    }),
  ],
  base: "",
  build: {
    target: "esnext", //browsers can handle the latest ES features
  },
  worker: {
    format: "es",
  },
});
