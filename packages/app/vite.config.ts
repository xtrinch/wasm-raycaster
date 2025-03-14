// packages/app/vite.config.ts
import wasm from "vite-plugin-wasm";

// export default defineConfig({
//   plugins: [wasm()],
//   build: {
//     outDir: "dist",
//     rollupOptions: {
//       input: "src/index.ts",
//       output: {
//         entryFileNames: "[name].js",
//         format: "esm",
//       },
//     },
//     target: "esnext",
//     minify: false,
//   },
// });

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react-swc";
import { defineConfig } from "vite";

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss(), wasm()],
  base: "raycasting-game",
  build: {
    target: "esnext", //browsers can handle the latest ES features
  },
});
