import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  Heart,
  Mic2,
  Pause,
  Play,
  Repeat,
  Repeat1,
  Shuffle,
  SkipBack,
  SkipForward,
  SlidersHorizontal,
  Volume2,
  VolumeX,
} from "lucide-react";
import { Slider } from "@/components/ui/slider";
import { StarRating } from "@/components/ui/star-rating";
import AlbumArt from "@/components/album-art";
import { usePlayerStore } from "@/lib/player-store";
import {
  getWaveform,
  lastfmGetStatus,
  lastfmIsLoved,
  lastfmLove,
  nextTrack,
  previousTrack,
  seekTo,
  setRating,
  setRepeat,
  setShuffle,
  setVolume,
  togglePlayPause,
  type RepeatMode,
} from "@/lib/ipc";
import { formatDuration } from "@/lib/format";
import { cn } from "@/lib/utils";
import { SleepTimerButton } from "@/components/sleep-timer-button";
import { useLyricsStore } from "@/lib/lyrics-store";
import { WaveformSeek } from "@/components/waveform-seek";

// ── Marquee text ──────────────────────────────────────────────────────────────

function MarqueeText({
  text,
  className,
}: {
  text: string;
  className?: string;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const textRef = useRef<HTMLSpanElement>(null);
  const [shift, setShift] = useState(0);

  useLayoutEffect(() => {
    const c = containerRef.current;
    const t = textRef.current;
    if (!c || !t) return;

    const measure = () => {
      const overflow = t.scrollWidth - c.offsetWidth;
      setShift(overflow > 4 ? overflow : 0);
    };

    measure();
    const obs = new ResizeObserver(measure);
    obs.observe(c);
    return () => obs.disconnect();
  }, [text]);

  return (
    <div ref={containerRef} className={cn("overflow-hidden min-w-0", className)}>
      <span
        ref={textRef}
        className="inline-block whitespace-nowrap"
        style={
          shift > 0
            ? ({
                animation: "marquee 7s ease-in-out 1.5s infinite alternate",
                "--marquee-shift": `-${shift}px`,
              } as React.CSSProperties)
            : undefined
        }
      >
        {text}
      </span>
    </div>
  );
}

// ── Mode button (shuffle / repeat) ────────────────────────────────────────────

function ModeButton({
  active,
  label,
  onClick,
  children,
}: {
  active: boolean;
  label: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      aria-label={label}
      aria-pressed={active}
      className={cn(
        "relative flex items-center justify-center w-7 h-7 rounded-md transition-colors",
        active
          ? "text-foreground bg-foreground/10"
          : "text-muted-foreground hover:text-foreground hover:bg-foreground/5",
      )}
    >
      {children}
      {active && (
        <span className="absolute bottom-0.5 left-1/2 -translate-x-1/2 w-1 h-1 rounded-full bg-primary" />
      )}
    </button>
  );
}

// ── Constants ─────────────────────────────────────────────────────────────────

const REPEAT_NEXT: Record<RepeatMode, RepeatMode> = {
  off: "all",
  all: "one",
  one: "off",
};

// ── Component ─────────────────────────────────────────────────────────────────

export default function NowPlaying() {
  const navigate = useNavigate();
  const lyricsOpen = useLyricsStore((s) => s.open);
  const toggleLyrics = useLyricsStore((s) => s.toggle);
  const init = usePlayerStore((s) => s.init);
  const status = usePlayerStore((s) => s.status);
  const position = usePlayerStore((s) => s.position_ms);
  const duration = usePlayerStore((s) => s.duration_ms);
  const volume = usePlayerStore((s) => s.volume);
  const shuffle = usePlayerStore((s) => s.shuffle);
  const repeat = usePlayerStore((s) => s.repeat);
  const track = usePlayerStore((s) => s.current_track);

  useEffect(() => { init(); }, [init]);

  const [scrub, setScrub] = useState<number | null>(null);
  const displayPosition = scrub ?? position;
  const max = duration ?? 0;
  const isPlaying = status === "playing";
  const hasTrack = track !== null;

  const onSeekChange = (val: number | readonly number[]) => {
    setScrub(Array.isArray(val) ? val[0] : (val as number));
  };
  const onSeekCommit = (val: number | readonly number[]) => {
    const next = Array.isArray(val) ? val[0] : (val as number);
    seekTo(next).catch(console.error);
    setScrub(null);
  };

  const composer = track?.composers?.join(", ") || null;
  const artist   = track?.artists.join(", ") || null;

  // Last.fm "love" state for the current track. The heart only appears when a
  // Last.fm session is connected; loved status is fetched on each track change.
  const trackId = track?.track_id ?? null;
  const [lfmConnected, setLfmConnected] = useState(false);
  const [loved, setLoved] = useState<boolean | null>(null);

  // Waveform peaks for the current track — fetched once per track change.
  // The backend computes + caches them lazily on first request.
  const [peaks, setPeaks] = useState<number[] | null>(null);
  useEffect(() => {
    let cancelled = false;
    setPeaks(null);
    if (trackId == null) return;
    getWaveform(trackId)
      .then((p) => { if (!cancelled) setPeaks(p.length > 0 ? p : null); })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [trackId]);

  useEffect(() => {
    let cancelled = false;
    if (trackId == null) {
      setLoved(null);
      return;
    }
    lastfmGetStatus()
      .then((s) => {
        if (cancelled) return;
        setLfmConnected(s.connected);
        if (s.connected) {
          lastfmIsLoved(trackId)
            .then((v) => { if (!cancelled) setLoved(v); })
            .catch(() => {});
        } else {
          setLoved(null);
        }
      })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [trackId]);

  const toggleLove = () => {
    if (trackId == null || loved == null) return;
    const next = !loved;
    setLoved(next); // optimistic; revert on failure
    lastfmLove(trackId, next).catch(() => setLoved(!next));
  };

  return (
    <div className="h-[88px] border-t border-border bg-background flex items-center px-4 gap-4 shrink-0">

      {/* ── Track info ──────────────────────────────────────────────────── */}
      <div className="flex items-center gap-3 w-80 shrink-0 min-w-0">
        <AlbumArt path={track?.album_art_path} size={40} rounded="sm" />
        <div className="min-w-0 flex-1">
          {/* Line 1: Title (static) · Album (marquee) */}
          <div className="flex items-center gap-1 overflow-hidden">
            <span
              className={cn(
                "text-sm font-medium whitespace-nowrap truncate shrink-0 max-w-[58%]",
                hasTrack ? "text-foreground" : "text-foreground/40",
              )}
            >
              {track?.title ?? "No track playing"}
            </span>
            {track?.album_name && (
              <>
                <span className="text-muted-foreground/50 text-xs shrink-0">·</span>
                <MarqueeText
                  text={track.album_name}
                  className="text-xs text-muted-foreground flex-1"
                />
              </>
            )}
          </div>

          {/* Line 2: Composer (static) · Artist (marquee) */}
          <div className="flex items-center gap-1 overflow-hidden mt-0.5">
            {composer ? (
              <>
                <span className="text-xs text-muted-foreground whitespace-nowrap truncate shrink-0 max-w-[58%]">
                  {composer}
                </span>
                {artist && (
                  <>
                    <span className="text-muted-foreground/50 text-xs shrink-0">·</span>
                    <MarqueeText
                      text={artist}
                      className="text-xs text-muted-foreground flex-1"
                    />
                  </>
                )}
              </>
            ) : artist ? (
              <MarqueeText
                text={artist}
                className="text-xs text-muted-foreground w-full"
              />
            ) : (
              <span className="text-xs text-muted-foreground">—</span>
            )}
          </div>
        </div>
      </div>

      {/* ── Playback controls ────────────────────────────────────────────── */}
      <div className="flex flex-col items-center gap-1 flex-1">
        <div className="flex items-center gap-2">
          {/* Rating before shuffle */}
          <StarRating
            value={track?.rating ?? 0}
            size={11}
            readonly={!hasTrack}
            onChange={(r) => {
              if (track) setRating(track.track_id, r).catch(console.error);
            }}
          />

          <ModeButton
            active={shuffle}
            label="Shuffle"
            onClick={() => setShuffle(!shuffle).catch(console.error)}
          >
            <Shuffle size={15} />
          </ModeButton>

          <button
            onClick={() => previousTrack().catch(console.error)}
            disabled={!hasTrack}
            className="text-muted-foreground hover:text-foreground disabled:opacity-40 transition-colors"
            aria-label="Previous"
          >
            <SkipBack size={18} />
          </button>

          <button
            onClick={() => togglePlayPause().catch(console.error)}
            disabled={!hasTrack && status === "stopped"}
            className="w-8 h-8 rounded-full bg-foreground text-background flex items-center justify-center hover:opacity-80 disabled:opacity-30 transition-opacity"
            aria-label={isPlaying ? "Pause" : "Play"}
          >
            {isPlaying
              ? <Pause size={14} />
              : <Play size={14} className="ml-0.5" />}
          </button>

          <button
            onClick={() => nextTrack().catch(console.error)}
            disabled={!hasTrack}
            className="text-muted-foreground hover:text-foreground disabled:opacity-40 transition-colors"
            aria-label="Next"
          >
            <SkipForward size={18} />
          </button>

          <ModeButton
            active={repeat !== "off"}
            label={`Repeat ${repeat}`}
            onClick={() => setRepeat(REPEAT_NEXT[repeat]).catch(console.error)}
          >
            {repeat === "one" ? <Repeat1 size={15} /> : <Repeat size={15} />}
          </ModeButton>
        </div>

        {/* Waveform seek bar with inline time labels */}
        <div className="flex items-center w-full max-w-xl gap-2">
          <span className="text-[10px] text-muted-foreground tabular-nums leading-none w-9 text-right shrink-0">
            {formatDuration(displayPosition)}
          </span>
          <div className="flex-1 h-7">
            <WaveformSeek
              peaks={peaks}
              position={displayPosition}
              duration={max}
              disabled={!hasTrack || max === 0}
              onSeekChange={(ms) => onSeekChange(ms)}
              onSeekCommit={(ms) => onSeekCommit(ms)}
            />
          </div>
          <span className="text-[10px] text-muted-foreground tabular-nums leading-none w-9 shrink-0">
            {formatDuration(duration)}
          </span>
        </div>
      </div>

      {/* ── Right controls ───────────────────────────────────────────────── */}
      <div className="flex items-center gap-2 w-44 shrink-0 justify-end">
        {lfmConnected && hasTrack && (
          <button
            onClick={toggleLove}
            disabled={loved == null}
            className={cn(
              "transition-colors disabled:opacity-40",
              loved
                ? "text-red-500 hover:text-red-400"
                : "text-muted-foreground hover:text-foreground",
            )}
            aria-label={loved ? "Unlove on Last.fm" : "Love on Last.fm"}
            aria-pressed={!!loved}
            title={loved ? "Loved on Last.fm" : "Love on Last.fm"}
          >
            <Heart size={16} fill={loved ? "currentColor" : "none"} />
          </button>
        )}
        <button
          onClick={() => navigate("/settings?tab=equalizer")}
          className="text-muted-foreground hover:text-foreground transition-colors"
          aria-label="Equalizer"
          title="Equalizer"
        >
          <SlidersHorizontal size={16} />
        </button>
        <button
          onClick={toggleLyrics}
          className={cn(
            "transition-colors",
            lyricsOpen
              ? "text-primary hover:text-primary/80"
              : "text-muted-foreground hover:text-foreground",
          )}
          aria-label="Toggle lyrics"
          aria-pressed={lyricsOpen}
        >
          <Mic2 size={16} />
        </button>
        <SleepTimerButton />
        <button
          onClick={() => setVolume(volume > 0 ? 0 : 0.75).catch(console.error)}
          className="text-muted-foreground hover:text-foreground transition-colors"
          aria-label={volume > 0 ? "Mute" : "Unmute"}
        >
          {volume > 0 ? <Volume2 size={16} /> : <VolumeX size={16} />}
        </button>
        <Slider
          value={[Math.round(volume * 100)]}
          max={100}
          onValueChange={(v) => {
            const next = Array.isArray(v) ? v[0] : (v as number);
            setVolume(next / 100).catch(console.error);
          }}
          className="w-20"
          aria-label="Volume"
        />
      </div>
    </div>
  );
}
