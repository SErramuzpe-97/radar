import { defineConfig } from "vite";
import preact from "@preact/preset-vite";

// En dev, la API vive en el binario Rust (127.0.0.1:7800); en producción el
// binario sirve esta UI embebida en el mismo origen.
export default defineConfig({
  plugins: [preact()],
  server: {
    port: 5173,
    proxy: {
      "/api": "http://127.0.0.1:7800",
      "/openapi.json": "http://127.0.0.1:7800",
    },
  },
  build: { outDir: "dist" },
});
