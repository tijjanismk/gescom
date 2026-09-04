import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";

export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
  ],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  // Sans ce préfixe, les variables TAURI_* ne sont pas transmises au
  // frontend : `import.meta.env` les ignore silencieusement.
  envPrefix: ["VITE_", "TAURI_"],
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    // WebView2 est un Chromium à jour : transpiler pour de vieux
    // navigateurs alourdit le bundle sans rien apporter.
    target: "chrome105",
    // Code lisible et sourcemaps uniquement en build de debug Tauri.
    minify: process.env.TAURI_ENV_DEBUG ? false : "esbuild",
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    // xlsx (SheetJS) dépasse à lui seul 500 kB : l'avertissement par
    // défaut se déclencherait à chaque build sans rien signaler
    // d'anormal.
    chunkSizeWarningLimit: 1200,
  },
});