import { useEffect } from "react";
import { useLocation } from "react-router-dom";

// Remembers scroll position per route (pathname + search) so navigating away
// and back — via the sidebar, a link, or the back button — restores where the
// user was. Lives at module scope so it survives component unmount/remount.
const store = new Map<string, number>();

/**
 * Persist and restore the scrollTop of `ref` keyed by the current route.
 *
 * @param ref   The scroll container (an element with overflow auto/scroll).
 * @param ready Whether the content is rendered. For virtualized lists pass
 *              `!loading && items.length > 0` so restoration waits until the
 *              list has height; otherwise scrollTop can't be set yet.
 */
export function useScrollMemory(
  ref: React.RefObject<HTMLElement | null>,
  ready: boolean = true,
  keyOverride?: string,
) {
  const { pathname, search } = useLocation();
  const key = keyOverride ?? (pathname + search);

  // Save scrollTop continuously while the user scrolls. `ready` is in the deps
  // because many pages only render their scroll container after data loads —
  // the listener must (re)attach once that element actually exists.
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const onScroll = () => store.set(key, el.scrollTop);
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => el.removeEventListener("scroll", onScroll);
  }, [ref, key, ready]);

  // Restore once the content is ready. Virtualized lists grow their height a
  // few frames after mount, so retry via rAF until the container is tall
  // enough to hold the target offset (or we give up after ~30 frames).
  useEffect(() => {
    const el = ref.current;
    if (!el || !ready) return;
    const target = store.get(key) ?? 0;
    if (target <= 0) {
      // Shared containers (reused across routes) must reset, or a previous
      // page's scroll would carry over to a fresh page.
      el.scrollTop = 0;
      return;
    }
    let raf = 0;
    let tries = 0;
    const restore = () => {
      const node = ref.current;
      if (!node) return;
      if (node.scrollHeight - node.clientHeight >= target || tries >= 30) {
        node.scrollTop = target;
        return;
      }
      tries++;
      raf = requestAnimationFrame(restore);
    };
    raf = requestAnimationFrame(restore);
    return () => cancelAnimationFrame(raf);
  }, [ref, key, ready]);
}
