import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [vue()],
  test: {
    environment: "jsdom",
    setupFiles: ["./tests/setup.js"],
  },
  server: {
    host: "127.0.0.1",
    port: 1421,
    strictPort: true,
  },
  clearScreen: false,
});
