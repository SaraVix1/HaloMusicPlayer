import { convertFileSrc } from "@tauri-apps/api/core";

// WebKitGTK 2.54 refuses to display asset:// URLs from an http:// page
// ("Unsafe attempt to load URL … Domains, protocols and ports must match"),
// which breaks every image in `tauri dev` on Linux — the dev page is served
// from http://localhost:1420. Production builds (tauri:// origin) are fine.
// In dev on Linux, serve files through Vite's same-origin /@fs/ route instead;
// vite.config.ts allow-lists Halo's art cache for it. See HAZARDS.md and
// https://github.com/tauri-apps/tauri/issues/16201. Remove once fixed upstream.
const useViteFs =
  import.meta.env.DEV && /Linux/.test(navigator.userAgent) && !/Android/.test(navigator.userAgent);

/** URL for displaying a local file (album art etc.) in the webview. */
export function assetUrl(path: string): string {
  if (useViteFs && path.startsWith("/")) {
    return `/@fs${encodeURI(path)}`;
  }
  return convertFileSrc(path);
}
