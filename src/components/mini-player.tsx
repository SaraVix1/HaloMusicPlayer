import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { Pause, Play, SkipBack, SkipForward } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import AlbumArt from "@/components/album-art";
import { Slider } from "@/components/ui/slider";
import { usePlayerStore } from "@/lib/player-store";
import { useThemeStore, applyTheme } from "@/lib/theme-store";
import {
  nextTrack,
  previousTrack,
  restoreMainWindow,
  saveMiniPosition,
  seekTo,
  setVolume,
  togglePlayPause,
} from "@/lib/ipc";
import { formatDuration } from "@/lib/format";
import { cn } from "@/lib/utils";

const MARQUEE_GAP = 48;
const MARQUEE_SPEED_PX_PER_SEC = 35;
const DRAG_MOVE_THRESHOLD = 4;
const ART_SIZE = 30;

function Marquee({ text, className }: { text: string; className?: string }) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const textRef = useRef<HTMLSpanElement>(null);
  const [shift, setShift] = useState<number | null>(null);

  useLayoutEffect(() => {
    const w = wrapRef.current;
    const t = textRef.current;
    if (!w || !t) {
      setShift(null);
      return;
    }
    const tw = t.offsetWidth;
    const ww = w.clientWidth;
    setShift(tw > ww + 1 ? tw + MARQUEE_GAP : null);
  }, [text]);

  const overflowing = shift !== null;
  const duration = overflowing ? Math.max(8, shift! / MARQUEE_SPEED_PX_PER_SEC) : 0;

  const style: CSSProperties | undefined = overflowing
    ? {
        gap: `${MARQUEE_GAP}px`,
        animation: `marquee ${duration}s linear infinite`,
        ["--marquee-shift" as string]: `-${shift}px`,
      }
    : undefined;

  return (
    <div ref={wrapRef} className={cn("overflow-hidden whitespace-nowrap", className)}>
      <div className="inline-flex" style={style}>
        <span ref={textRef}>{text}</span>
        {overflowing && <span aria-hidden>{text}</span>}
      </div>
    </div>
  );
}

function VolumeFillBar() {
  const volume = usePlayerStore((s) => s.volume);
  const ref = useRef<HTMLDivElement>(null);

  const updateFromY = (clientY: number) => {
    const el = ref.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const ratio = 1 - (clientY - rect.top) / rect.height;
    const next = Math.max(0, Math.min(1, ratio));
    setVolume(next).catch(console.error);
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
    const el = ref.current;
    if (!el) return;
    el.setPointerCapture(e.pointerId);
    updateFromY(e.clientY);
  };

  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (e.buttons !== 1) return;
    updateFromY(e.clientY);
  };

  const pct = Math.round(volume * 100);

  return (
    <div
      ref={ref}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      role="slider"
      aria-label="Volume"
      aria-valuenow={pct}
      aria-valuemin={0}
      aria-valuemax={100}
      className="relative shrink-0 w-1.5 h-full bg-muted cursor-pointer"
    >
      <div
        className="absolute inset-x-0 bottom-0 bg-primary pointer-events-none"
        style={{ height: `${pct}%` }}
      />
    </div>
  );
}

export default function MiniPlayer() {
  const initPlayer = usePlayerStore((s) => s.init);
  const initTheme = useThemeStore((s) => s.init);
  const status = usePlayerStore((s) => s.status);
  const track = usePlayerStore((s) => s.current_track);
  const position = usePlayerStore((s) => s.position_ms);
  const duration = usePlayerStore((s) => s.duration_ms);

  useEffect(() => {
    initTheme();
    initPlayer();
  }, [initPlayer, initTheme]);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;
    listen<string>("theme-changed", (event) => {
      applyTheme(event.payload as import("@/lib/ipc").Theme);
    })
      .then((un) => {
        if (cancelled) un();
        else unlisten = un;
      })
      .catch(console.error);
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "m") {
        e.preventDefault();
        restoreMainWindow().catch(console.error);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let timer: number | undefined;
    let cancelled = false;
    getCurrentWindow()
      .onMoved(({ payload }) => {
        if (timer !== undefined) window.clearTimeout(timer);
        timer = window.setTimeout(() => {
          saveMiniPosition(payload.x, payload.y).catch(console.error);
        }, 300);
      })
      .then((un) => {
        if (cancelled) un();
        else unlisten = un;
      })
      .catch(console.error);
    return () => {
      cancelled = true;
      if (timer !== undefined) window.clearTimeout(timer);
      if (unlisten) unlisten();
    };
  }, []);

  const [scrub, setScrub] = useState<number | null>(null);
  const [hovered, setHovered] = useState(false);
  const contentRef = useRef<HTMLDivElement>(null);

  const displayPosition = scrub ?? position;
  const max = duration ?? 0;

  useEffect(() => {
    const onVis = () => {
      if (document.visibilityState !== "visible") setHovered(false);
    };
    document.addEventListener("visibilitychange", onVis);
    return () => document.removeEventListener("visibilitychange", onVis);
  }, []);

  useEffect(() => {
    // contentRef's own onMouseLeave is a DOM bounding-box event. On Linux/
    // WebKitGTK, pointer-leave delivery near a small window's edges is
    // unreliable depending on what's outside that edge (another window vs.
    // open desktop) — observed to only fire reliably leaving via the bottom
    // (open space below the window), not top/left/right. relatedTarget ===
    // null on a document-level mouseout is the robust cross-platform signal
    // for "the pointer left the window/document entirely", independent of
    // which edge was crossed.
    const onDocMouseOut = (e: MouseEvent) => {
      if (e.relatedTarget === null) setHovered(false);
    };
    document.addEventListener("mouseout", onDocMouseOut);
    return () => document.removeEventListener("mouseout", onDocMouseOut);
  }, []);

  useEffect(() => {
    // Fallback of last resort: on this WebKitGTK build, neither element-level
    // mouseleave nor document-level mouseout(relatedTarget=null) reliably
    // fire when the cursor exits through the top/left/right edges — the
    // compositor apparently never delivers a pointer-leave to the surface in
    // those directions. Without any leave event, detect "the cursor stopped
    // generating movement" instead: while hovered, any mousemove resets a
    // timer; if no movement arrives for IDLE_MS, assume the cursor left and
    // revert to the title view. Mirrors the existing accumulated-movement
    // trick used to gate hover *on* (see the sticky-hover hazard in
    // docs/HAZARDS.md) — same "don't trust native enter/leave" philosophy,
    // applied to the leave side. Confirmed via diagnostic logging: mousemove
    // does stop once the cursor leaves in every direction, this was just
    // never wired to actually fire before — it now reverts ~IDLE_MS after
    // leaving via top/left/right (vs. instant via the bottom, where the real
    // mouseleave still fires).
    if (!hovered) return;
    const IDLE_MS = 600;
    let lastMove = Date.now();
    const onMove = () => {
      lastMove = Date.now();
    };
    window.addEventListener("mousemove", onMove);
    const interval = window.setInterval(() => {
      if (Date.now() - lastMove > IDLE_MS) setHovered(false);
    }, 150);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.clearInterval(interval);
    };
  }, [hovered]);

  const onContentMouseMove = (e: React.MouseEvent) => {
    if (!contentRef.current) return;
    if (hovered) return;
    const rect = contentRef.current.getBoundingClientRect();
    setHovered(e.clientX - rect.left > ART_SIZE);
  };

  const isPlaying = status === "playing";
  const hasTrack = track !== null;

  const restore = () => restoreMainWindow().catch(console.error);

  const onExpandPointerDown = (e: ReactPointerEvent<HTMLButtonElement>) => {
    if (e.button !== 0) return;
    const startX = e.clientX;
    const startY = e.clientY;
    let dragged = false;
    const onMove = (ev: PointerEvent) => {
      if (dragged) return;
      if (
        Math.abs(ev.clientX - startX) > DRAG_MOVE_THRESHOLD ||
        Math.abs(ev.clientY - startY) > DRAG_MOVE_THRESHOLD
      ) {
        dragged = true;
        cleanup();
        getCurrentWindow().startDragging().catch(console.error);
      }
    };
    const onUp = () => {
      cleanup();
      if (!dragged) restore();
    };
    const cleanup = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  };

  const onSeekChange = (val: number | readonly number[]) => {
    const next = Array.isArray(val) ? val[0] : (val as number);
    setScrub(next);
  };
  const onSeekCommit = (val: number | readonly number[]) => {
    const next = Array.isArray(val) ? val[0] : (val as number);
    seekTo(next).catch(console.error);
    setScrub(null);
  };

  const composerText = track?.composers?.join(", ") || null;
  const artistText  = track?.artists.join(", ") || "—";
  const albumText   = track?.album_name ?? "";

  const progressPct = max > 0 ? Math.min(100, (position / max) * 100) : 0;

  return (
    <div
      data-tauri-drag-region
      onDoubleClick={restore}
      className="relative h-[30px] w-full bg-background text-foreground select-none overflow-hidden flex"
    >
      <VolumeFillBar />

      <div
        ref={contentRef}
        data-tauri-drag-region
        onMouseMove={onContentMouseMove}
        onMouseLeave={() => setHovered(false)}
        className="relative flex-1 min-w-0 h-full"
      >
        {/* Default layer: album art + title + artist · album + red progress bar (read-only, after image) */}
        <div
          data-tauri-drag-region
          className={cn(
            "absolute inset-0 flex items-center gap-2 transition-opacity duration-150 pointer-events-none",
            hovered ? "opacity-0" : "opacity-100",
          )}
        >
          <AlbumArt path={track?.album_art_path} size={ART_SIZE} className="!rounded-none" />
          <div className="flex flex-col flex-1 min-w-0 justify-center pb-[2px]">
            {/* Line 1: Title (static) · Album (marquee) */}
            <div className="flex items-center gap-1 min-w-0 leading-[13px]">
              <span
                className={cn(
                  "text-[11px] font-medium truncate shrink-0 max-w-[58%]",
                  hasTrack ? "text-foreground" : "text-foreground/40",
                )}
              >
                {track?.title ?? "No track playing"}
              </span>
              {albumText && (
                <>
                  <span className="text-[9px] text-muted-foreground/60 shrink-0">·</span>
                  <Marquee text={albumText} className="text-[9px] text-muted-foreground flex-1 min-w-0" />
                </>
              )}
            </div>
            {/* Line 2: Composer (static) · Artist (marquee) */}
            <div className="flex items-center gap-1 min-w-0 leading-[11px] mt-px">
              {composerText ? (
                <>
                  <span className="text-[9px] text-muted-foreground truncate shrink-0 max-w-[58%]">
                    {composerText}
                  </span>
                  {artistText !== "—" && (
                    <>
                      <span className="text-[9px] text-muted-foreground/60 shrink-0">·</span>
                      <Marquee text={artistText} className="text-[9px] text-muted-foreground flex-1 min-w-0" />
                    </>
                  )}
                </>
              ) : (
                <Marquee text={artistText} className="text-[9px] text-muted-foreground flex-1 min-w-0" />
              )}
            </div>
          </div>
        </div>

        {/* Control layer: prev/play/next + time/slider/time */}
        <div
          data-tauri-drag-region
          className={cn(
            "absolute inset-0 flex items-center gap-2 pl-1.5 transition-opacity duration-150",
            hovered ? "opacity-100" : "opacity-0 pointer-events-none",
          )}
        >
          <div className="flex items-center gap-1.5 shrink-0">
            <button
              onClick={() => previousTrack().catch(console.error)}
              disabled={!hasTrack}
              className="text-muted-foreground hover:text-foreground disabled:opacity-40 transition-colors"
              aria-label="Previous"
            >
              <SkipBack size={13} />
            </button>
            <button
              onClick={() => togglePlayPause().catch(console.error)}
              disabled={!hasTrack && status === "stopped"}
              className="w-5 h-5 rounded-full bg-foreground text-background flex items-center justify-center hover:opacity-80 disabled:opacity-30 transition-opacity"
              aria-label={isPlaying ? "Pause" : "Play"}
            >
              {isPlaying ? <Pause size={10} /> : <Play size={10} className="ml-0.5" />}
            </button>
            <button
              onClick={() => nextTrack().catch(console.error)}
              disabled={!hasTrack}
              className="text-muted-foreground hover:text-foreground disabled:opacity-40 transition-colors"
              aria-label="Next"
            >
              <SkipForward size={13} />
            </button>
          </div>
          <div className="flex items-center gap-1 flex-1 min-w-0">
            <span data-tauri-drag-region className="text-[9px] text-muted-foreground tabular-nums w-7 text-right">
              {formatDuration(displayPosition)}
            </span>
            <Slider
              value={[Math.min(displayPosition, max || displayPosition)]}
              max={max || 1}
              disabled={!hasTrack || max === 0}
              onValueChange={onSeekChange}
              onValueCommitted={onSeekCommit}
              className="flex-1"
              aria-label="Seek"
            />
            <span data-tauri-drag-region className="text-[9px] text-muted-foreground tabular-nums w-7">
              {formatDuration(duration)}
            </span>
          </div>
        </div>
      </div>

      <button
        onPointerDown={onExpandPointerDown}
        className="shrink-0 w-3 h-full flex items-center justify-center text-muted-foreground hover:text-foreground bg-muted/40 hover:bg-muted/70 transition-colors"
        aria-label="Restore full window"
        title="Restore full window (Ctrl+M)"
      >
        <svg viewBox="0 0 6 8" width="6" height="8" aria-hidden>
          <path d="M0 0 L6 4 L0 8 Z" fill="currentColor" />
        </svg>
      </button>

      {/* Red progress bar — flush after image, extends to window right edge. Image view only, read-only. */}
      <div
        className={cn(
          "absolute bottom-0 right-0 h-[2px] bg-red-900/30 pointer-events-none transition-opacity duration-150",
          hovered ? "opacity-0" : "opacity-100",
        )}
        style={{ left: `${6 + ART_SIZE}px` }}
      >
        <div
          className="absolute inset-y-0 left-0 bg-red-500"
          style={{ width: `${progressPct}%` }}
        />
      </div>
    </div>
  );
}
