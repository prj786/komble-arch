import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";

// Tailwind 4 is a first-class Vite plugin — no postcss.config.js, and the
// theme lives in app.css under @theme rather than in tailwind.config.js.
export default defineConfig({
  plugins: [tailwindcss(), svelte()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] }
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    target: "chrome105",
    // Vite 8 builds with rolldown/oxc; "esbuild" is deprecated there and
    // would require esbuild as a separate dependency.
    minify: true,
    sourcemap: false
  }
});
