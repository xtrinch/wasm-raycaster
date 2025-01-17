// packages/app/vite.config.ts
import { defineConfig } from 'vite'
import wasm from 'vite-plugin-wasm'

export default defineConfig({
    plugins: [wasm()],
    build: {
        outDir: 'dist',
        rollupOptions: {
            input: 'src/index.ts',
            output: {
                entryFileNames: '[name].js',
                format: 'esm',
            }
        },
        target: 'esnext',
        minify: false
    }
})