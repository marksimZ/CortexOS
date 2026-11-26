import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tauriConfig from "./src-tauri/tauri.conf.json" with { type: "json" };

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
    headers: { "Content-Security-Policy": tauriConfig.app.security.devCsp },
    watch: { ignored: ["**/src-tauri/**"] },
    fs: { strict: true },
  },
  preview: { host: "127.0.0.1", port: 1420, strictPort: true },
  build: {
    target: "chrome105",
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [
            {
              name: "react-vendor",
              test: /[\\/]node_modules[\\/](react|react-dom|scheduler)[\\/]/,
            },
            {
              name: "markdown-vendor",
              test: /[\\/]node_modules[\\/](react-markdown|remark-|rehype-|micromark|mdast|hast|unist|vfile|property-information|comma-separated-tokens|space-separated-tokens|decode-named-character-reference|character-entities)/,
            },
            {
              name: "icons-vendor",
              test: /[\\/]node_modules[\\/]lucide-react[\\/]/,
            },
          ],
        },
      },
    },
  },
});
