import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Play, Pause, X, Trash2 } from "lucide-react";
import {
  clearQueue,
  getQueue,
  playQueueIndex,
  removeFromQueue,
  togglePlayPause,
  type QueueTrack,
} from "@/lib/ipc";
import { usePlayerStore } from "@/lib/player-store";
import { formatDuration } from "@/lib/format";
import AlbumArt from "@/components/album-art";
import { useScrollMemory } from "@/lib/scroll-memory";
import { cn } from "@/lib/utils";

const ROW_HEIGHT = 48;

export default function Queue() {
  const [items, setItems] = useState<QueueTrack[]>([]);
  const [loading, setLoading] = useState(true);
  const currentIndex = usePlayerStore((s) => s.current_index);
  const status = usePlayerStore((s) => s.status);

  const refresh = () => {
    getQueue()
      .then(setItems)
      .catch(console.error)
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    refresh();
    let lastLength = -1;
    const unlistenPromise = listen<{ queue_length: number }>(
      "player-state",
      (event) => {
        // Only refresh when queue length changes — avoid 4Hz queue reload thrash.
        const len = event.payload.queue_length;
        if (len !== lastLength) {
          lastLength = len;
          refresh();
        }
      },
    );
    return () => {
      unlistenPromise.then((u) => u());
    };
  }, []);

  const onRowClick = (idx: number) => {
    if (idx === currentIndex) {
      togglePlayPause().catch(console.error);
    } else {
      playQueueIndex(idx).catch(console.error);
    }
  };

  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 10,
  });
  useScrollMemory(parentRef, !loading && items.length > 0);

  return (
    <div className="p-6 h-full flex flex-col">
      <div className="flex items-baseline justify-between gap-4 mb-4 shrink-0">
        <h1 className="text-2xl font-semibold">Queue</h1>
        <div className="flex items-center gap-3">
          <span className="text-sm text-muted-foreground tabular-nums">
            {items.length} {items.length === 1 ? "track" : "tracks"}
          </span>
          {items.length > 0 && (
            <button
              onClick={() => clearQueue().catch(console.error)}
              className="inline-flex items-center gap-1.5 rounded-md border border-border px-2.5 py-1 text-xs hover:bg-muted/40"
            >
              <Trash2 size={12} />
              Clear
            </button>
          )}
        </div>
      </div>

      {loading ? (
        <p className="text-sm text-muted-foreground">Loading…</p>
      ) : items.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          Queue is empty. Play a track from the library to get started.
        </p>
      ) : (
        <div
          ref={parentRef}
          className="rounded-md border border-border overflow-auto flex-1 min-h-0"
        >
          <div
            style={{ height: virtualizer.getTotalSize(), position: "relative" }}
          >
            {virtualizer.getVirtualItems().map((item) => {
              const row = items[item.index];
              const isCurrent = item.index === currentIndex;
              const isPlaying = isCurrent && status === "playing";
              return (
                <div
                  key={row.queue_id}
                  style={{
                    position: "absolute",
                    top: 0,
                    left: 0,
                    width: "100%",
                    height: item.size,
                    transform: `translateY(${item.start}px)`,
                  }}
                  className={cn(
                    "flex items-center gap-3 px-3 border-t border-border first:border-t-0 group",
                    isCurrent && "bg-muted/40",
                  )}
                >
                  <button
                    onClick={() => onRowClick(item.index)}
                    className="w-8 h-8 flex items-center justify-center text-muted-foreground hover:text-foreground"
                    aria-label={isPlaying ? "Pause" : "Play"}
                  >
                    {isPlaying ? <Pause size={14} /> : <Play size={14} />}
                  </button>
                  <AlbumArt path={row.album_art_path} size={32} rounded="sm" />
                  <div className="min-w-0 flex-1">
                    <p
                      className={cn(
                        "text-sm truncate",
                        isCurrent && "font-medium text-foreground",
                      )}
                    >
                      {row.title ?? row.file_path.split(/[\\/]/).pop()}
                    </p>
                    <p className="text-xs truncate text-muted-foreground">
                      {row.artists.join(", ") || "—"}
                      {row.album_name && ` · ${row.album_name}`}
                    </p>
                  </div>
                  <span className="text-xs text-muted-foreground tabular-nums w-12 text-right">
                    {formatDuration(row.duration_ms)}
                  </span>
                  <button
                    onClick={() =>
                      removeFromQueue(row.queue_id).catch(console.error)
                    }
                    className="w-7 h-7 flex items-center justify-center text-muted-foreground hover:text-foreground opacity-0 group-hover:opacity-100 transition-opacity"
                    aria-label="Remove from queue"
                  >
                    <X size={14} />
                  </button>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
