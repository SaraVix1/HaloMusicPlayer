import { useCallback, useEffect, useRef, useState } from "react";
import { cn } from "@/lib/utils";
import { formatDuration } from "@/lib/format";

// ── Waveform seek bar ─────────────────────────────────────────────────────────
//
// Renders a track's precomputed loudness peaks (0..255 buckets) on a canvas,
// splitting played (primary) from upcoming (muted) at the current position.
// Click / drag to seek; hover shows the timestamp under the cursor. Falls back
// to a flat baseline when no peaks are available yet, so seeking always works.

const SEEK_STEP_MS = 5000; // arrow-key nudge

function readColor(el: HTMLElement | null): string {
  if (!el) return "#888";
  return getComputedStyle(el).color || "#888";
}

export function WaveformSeek({
  peaks,
  position,
  duration,
  disabled = false,
  onSeekChange,
  onSeekCommit,
  className,
}: {
  peaks: number[] | null;
  position: number;
  duration: number;
  disabled?: boolean;
  onSeekChange: (ms: number) => void;
  onSeekCommit: (ms: number) => void;
  className?: string;
}) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const playedProbe = useRef<HTMLSpanElement>(null);
  const restProbe = useRef<HTMLSpanElement>(null);

  const [dragging, setDragging] = useState(false);
  const [hoverX, setHoverX] = useState<number | null>(null);

  const max = duration > 0 ? duration : 0;
  const fraction = max > 0 ? Math.min(1, Math.max(0, position / max)) : 0;

  // ── Draw ────────────────────────────────────────────────────────────────────
  const draw = useCallback(() => {
    const canvas = canvasRef.current;
    const wrap = wrapRef.current;
    if (!canvas || !wrap) return;

    const dpr = window.devicePixelRatio || 1;
    const w = wrap.clientWidth;
    const h = wrap.clientHeight;
    if (w === 0 || h === 0) return;

    if (canvas.width !== w * dpr || canvas.height !== h * dpr) {
      canvas.width = w * dpr;
      canvas.height = h * dpr;
    }
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);

    const playedColor = readColor(playedProbe.current);
    const restColor = readColor(restProbe.current);

    const bars = peaks && peaks.length > 0 ? peaks : null;
    const count = bars ? bars.length : Math.max(1, Math.floor(w / 3));
    const slot = w / count;
    const barW = Math.max(1, slot * 0.6);
    const playedX = w * fraction;
    const mid = h / 2;

    for (let i = 0; i < count; i++) {
      const x = i * slot + (slot - barW) / 2;
      // Normalized 0..1 amplitude → bar half-height. Flat baseline when no peaks.
      const amp = bars ? bars[i] / 255 : 0.12;
      const barH = Math.max(2, amp * (h - 2));
      ctx.fillStyle = x + barW <= playedX ? playedColor : restColor;
      ctx.fillRect(x, mid - barH / 2, barW, barH);
    }
  }, [peaks, fraction]);

  useEffect(() => {
    draw();
    const wrap = wrapRef.current;
    if (!wrap) return;
    const obs = new ResizeObserver(draw);
    obs.observe(wrap);
    return () => obs.disconnect();
  }, [draw]);

  // ── Pointer → time ────────────────────────────────────────────────────────────
  const msAtClientX = useCallback(
    (clientX: number): number => {
      const wrap = wrapRef.current;
      if (!wrap || max === 0) return 0;
      const rect = wrap.getBoundingClientRect();
      const frac = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
      return Math.round(frac * max);
    },
    [max],
  );

  const onPointerDown = (e: React.PointerEvent) => {
    if (disabled || max === 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    setDragging(true);
    onSeekChange(msAtClientX(e.clientX));
  };

  const onPointerMove = (e: React.PointerEvent) => {
    if (max === 0) return;
    setHoverX(e.clientX - (wrapRef.current?.getBoundingClientRect().left ?? 0));
    if (dragging) onSeekChange(msAtClientX(e.clientX));
  };

  const onPointerUp = (e: React.PointerEvent) => {
    if (!dragging) return;
    setDragging(false);
    onSeekCommit(msAtClientX(e.clientX));
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (disabled || max === 0) return;
    if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      e.preventDefault();
      const delta = e.key === "ArrowLeft" ? -SEEK_STEP_MS : SEEK_STEP_MS;
      onSeekCommit(Math.min(max, Math.max(0, position + delta)));
    }
  };

  const hoverMs =
    hoverX != null && wrapRef.current
      ? Math.round(
          Math.min(1, Math.max(0, hoverX / wrapRef.current.clientWidth)) * max,
        )
      : null;

  return (
    <div
      ref={wrapRef}
      role="slider"
      tabIndex={disabled ? -1 : 0}
      aria-label="Seek"
      aria-valuemin={0}
      aria-valuemax={max}
      aria-valuenow={Math.round(position)}
      aria-valuetext={`${formatDuration(position)} of ${formatDuration(duration)}`}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerLeave={() => setHoverX(null)}
      onKeyDown={onKeyDown}
      className={cn(
        "relative h-full w-full cursor-pointer select-none outline-none focus-visible:ring-1 focus-visible:ring-ring rounded-sm",
        disabled && "opacity-50 pointer-events-none",
        className,
      )}
    >
      <canvas ref={canvasRef} className="block h-full w-full" />

      {/* Hidden colour probes — let canvas read the active theme tokens. */}
      <span ref={playedProbe} className="text-primary hidden" aria-hidden />
      <span ref={restProbe} className="text-muted-foreground/40 hidden" aria-hidden />

      {/* Hover timestamp */}
      {hoverMs != null && !disabled && (
        <div
          className="pointer-events-none absolute -top-5 -translate-x-1/2 rounded bg-popover px-1 py-0.5 text-[10px] tabular-nums text-popover-foreground shadow"
          style={{ left: `${hoverX}px` }}
        >
          {formatDuration(hoverMs)}
        </div>
      )}
    </div>
  );
}
