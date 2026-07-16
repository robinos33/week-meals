import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { VitePWA } from "vite-plugin-pwa";

// Couleur de thème « Cantine » (vert potager) — doit rester alignée sur le
// token `--color-primary` du thème clair (src/theme/tokens.css).
const THEME_GREEN = "#3f7d54";
const CANTINE_CREAM = "#faf7f0";

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    react(),
    VitePWA({
      // `prompt` : on notifie l'utilisateur qu'une mise à jour est prête
      // (géré dans src/pwa.ts) plutôt que de recharger dans son dos.
      registerType: "prompt",
      includeAssets: ["favicon.svg", "apple-touch-icon-180.png"],
      manifest: {
        name: "Week Meals",
        short_name: "Week Meals",
        description:
          "Recettes, planning des repas de la semaine et liste de courses intelligente.",
        lang: "fr",
        dir: "ltr",
        start_url: "/",
        scope: "/",
        display: "standalone",
        orientation: "portrait",
        background_color: CANTINE_CREAM,
        theme_color: THEME_GREEN,
        categories: ["food", "lifestyle", "productivity"],
        icons: [
          { src: "icons/icon-192.png", sizes: "192x192", type: "image/png" },
          { src: "icons/icon-512.png", sizes: "512x512", type: "image/png" },
          {
            src: "icons/maskable-512.png",
            sizes: "512x512",
            type: "image/png",
            purpose: "maskable",
          },
        ],
      },
      workbox: {
        globPatterns: ["**/*.{js,css,html,svg,png,ico,woff2}"],
        // Le shell applicatif est servi hors-ligne ; l'API reste online-only
        // (seule la liste de courses passera offline, cf. ADR-0004).
        navigateFallback: "/index.html",
        cleanupOutdatedCaches: true,
      },
      devOptions: {
        // Permet de tester le SW en `vite dev` si besoin.
        enabled: false,
      },
    }),
  ],
});
