import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Settings recommended by Tauri: fixed port, don't clear the terminal,
// don't watch the Rust folder.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**", "**/pm/**"] },
  },
});
