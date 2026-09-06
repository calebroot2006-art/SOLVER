import process from "node:process";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Set by `tauri dev` when developing against a device on the LAN. Undefined for the
// ordinary desktop run, which keeps the dev server on loopback.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],

  // Rust errors scroll past if Vite clears the screen under `tauri dev`.
  clearScreen: false,

  server: {
    // tauri.conf.json's devUrl names this port; fail rather than silently move.
    port: 1420,
    strictPort: true,
    host: host ?? false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
