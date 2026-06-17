import { useState, useRef, useEffect } from "react";
import { Moon, X } from "lucide-react";
import { usePlayerStore } from "@/lib/player-store";
import { setSleepTimer, setSleepTimerEndOfSong, cancelSleepTimer } from "@/lib/ipc";
import { cn } from "@/lib/utils";

const PRESETS = [5, 10, 15, 30, 45, 60];

function formatCountdown(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  if (h > 0)
    return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  return `${m}:${String(s).padStart(2, "0")}`;
}

export function SleepTimerButton() {
  const sleepTimer = usePlayerStore((s) => s.sleep_timer);
  const [open, setOpen] = useState(false);
  const [fade, setFade] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  const active = sleepTimer.active;

  const handlePreset = (minutes: number) => {
    setSleepTimer(minutes, fade).catch(console.error);
    setOpen(false);
  };

  const handleEndOfSong = () => {
    setSleepTimerEndOfSong(fade).catch(console.error);
    setOpen(false);
  };

  const handleCancel = (e: React.MouseEvent) => {
    e.stopPropagation();
    cancelSleepTimer().catch(console.error);
  };

  return (
    <div ref={containerRef} className="relative">
      <button
        onClick={() => setOpen((o) => !o)}
        className={cn(
          "flex items-center gap-1 transition-colors text-xs tabular-nums",
          active
            ? "text-primary hover:text-primary/80"
            : "text-muted-foreground hover:text-foreground",
        )}
        aria-label="Sleep timer"
        title="Sleep timer"
      >
        <Moon size={15} />
        {active && sleepTimer.end_of_song && (
          <span className="leading-none">EOS</span>
        )}
        {active && !sleepTimer.end_of_song && sleepTimer.remaining_secs !== null && (
          <span className="leading-none">
            {formatCountdown(sleepTimer.remaining_secs)}
          </span>
        )}
      </button>

      {open && (
        <div className="absolute bottom-full right-0 mb-2 w-52 rounded-lg border border-border bg-popover shadow-lg z-50 p-3">
          <div className="flex items-center justify-between mb-2.5">
            <span className="text-xs font-medium">Sleep timer</span>
            {active && (
              <button
                onClick={handleCancel}
                className="flex items-center gap-1 text-xs text-destructive hover:text-destructive/80 transition-colors"
              >
                <X size={11} />
                Cancel
              </button>
            )}
          </div>

          <div className="grid grid-cols-3 gap-1.5 mb-2.5">
            {PRESETS.map((m) => {
              const isActive =
                active &&
                !sleepTimer.end_of_song &&
                sleepTimer.remaining_secs !== null &&
                Math.abs(sleepTimer.remaining_secs - m * 60) < 30;
              return (
                <button
                  key={m}
                  onClick={() => handlePreset(m)}
                  className={cn(
                    "rounded px-2 py-1 text-xs transition-colors border",
                    isActive
                      ? "bg-primary text-primary-foreground border-primary"
                      : "border-border hover:bg-muted",
                  )}
                >
                  {m}m
                </button>
              );
            })}
          </div>

          <button
            onClick={handleEndOfSong}
            className={cn(
              "w-full rounded px-2 py-1.5 text-xs text-left transition-colors border mb-2.5",
              active && sleepTimer.end_of_song
                ? "bg-primary text-primary-foreground border-primary"
                : "border-border hover:bg-muted",
            )}
          >
            End of current song
          </button>

          <label className="flex items-center gap-2 text-xs text-muted-foreground cursor-pointer select-none">
            <input
              type="checkbox"
              checked={fade}
              onChange={(e) => setFade(e.target.checked)}
              className="accent-primary"
            />
            Fade out
          </label>
        </div>
      )}
    </div>
  );
}
