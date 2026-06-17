import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

/**
 * Subscribe to backend `library-changed` events (emitted after a scan,
 * clear-cache, or clear-database). Returns a counter that increments on
 * each event — include it in a `useEffect` dependency array to re-fetch
 * data when the library mutates.
 */
export function useLibraryRefresh(): number {
  const [tick, setTick] = useState(0);
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listen("library-changed", () => {
      setTick((n) => n + 1);
    }).then((u) => {
      if (cancelled) u();
      else unlisten = u;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);
  return tick;
}
