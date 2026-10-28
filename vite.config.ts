import { defineConfig, searchForWorkspaceRoot } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";
import os from "os";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// Halo's art cache ($APPCACHE on Linux). In dev on Linux, album art is served
// through Vite's /@fs/ route instead of asset:// — WebKitGTK 2.54 blocks
// asset:// images from the http:// dev origin. See src/lib/asset-url.ts.
const xdgCache: string = process.env.XDG_CACHE_HOME || path.join(os.homedir(), ".cache");
const appCacheDir = path.join(xdgCache, "com.saravix.halo");

export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],

  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },

  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
    fs: {
      allow: [searchForWorkspaceRoot(process.cwd()), appCacheDir],
    },
  },
}));
