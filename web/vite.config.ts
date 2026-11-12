import { defineConfig } from "vite";

export default defineConfig({
  server: {
    host: "127.0.0.1",
    port: 5173,
    // The tunnel must never silently point at a different app on port 5173.
    strictPort: true,
    proxy: {
      "/ws": {
        target: "ws://127.0.0.1:8080",
        ws: true,
        // Rust currently accepts WebSocket upgrades at /.
        rewrite: () => "/",
      },
    },
  },
});
