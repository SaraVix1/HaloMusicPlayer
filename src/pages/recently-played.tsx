import { useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ListPlus, Pencil, Play, Plus, Shuffle } from "lucide-react";
import {
  addToQueue,
  getRecentlyPlayed,
  setQueueAndPlay,
  setRating,
  setShuffle,
  type Track,
} from "@/lib/ipc";
import { formatDuration } from "@/lib/format";
import AlbumArt from "@/components/album-art";
import AddToPlaylistDialog from "@/components/add-to-playlist-dialog";
import MetadataEditorDialog from "@/components/metadata-editor-dialog";
import { StarRating } from "@/components/ui/star-rating";
import { usePlayerStore } from "@/lib/player-store";
import { useLibraryRefresh } from "@/lib/library-events";
import { useScrollMemory } from "@/lib/scroll-memory";
import { cn } from "@/lib/utils";

const ROW_HEIGHT = 44;
const GRID_COLS =
  "grid-cols-[48px_48px_minmax(0,2fr)_minmax(0,1.5fr)_minmax(0,1.5fr)_minmax(0,1fr)_80px_100px_72px]";

function formatLastPlayed(ts: number | null): string {
  if (!ts) return "—";
  const date = new Date(ts * 1000);
  const now = Date.now();
  const diffMs = now - date.getTime();
  const diffSecs = Math.floor(diffMs / 1000);
  if (diffSecs < 60) return "Just now";
  const diffMins = Math.floor(diffSecs / 60);
  if (diffMins < 60) return `${diffMins}m ago`;
  const diffHours = Math.floor(diffMins / 60);
  if (diffHours < 24) return `${diffHours}h ago`;
  const diffDays = Math.floor(diffHours / 24);
  if (diffDays < 7) return `${diffDays}d ago`;
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
}

export default function RecentlyPlayed() {
  const [tracks, setTracks] = useState<Track[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const refreshTick = useLibraryRefresh();

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    getRecentlyPlayed()
      .then((rows) => { if (!cancelled) setTracks(rows); })
      .catch((e) => { if (!cancelled) setError(String(e)); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [refreshTick]);

  const playFromRow = (startIndex: number) => {
    if (!tracks.length) return;
    setQueueAndPlay(tracks.map((t) => t.id), startIndex).catch(console.error);
  };

  const playAll = () => {
    if (!tracks.length) return;
    setShuffle(false).catch(console.error);
    setQueueAndPlay(tracks.map((t) => t.id), 0).catch(console.error);
  };

  const shufflePlay = () => {
    if (!tracks.length) return;
    setShuffle(true).catch(console.error);
    const start = Math.floor(Math.random() * tracks.length);
    setQueueAndPlay(tracks.map((t) => t.id), start).catch(console.error);
  };

  const handleRating = (trackId: number, rating: number) => {
    setTracks((prev) => prev.map((t) => (t.id === trackId ? { ...t, rating } : t)));
    setRating(trackId, rating).catch(console.error);
  };

  const handleMetaSaved = () => {
    getRecentlyPlayed().then(setTracks).catch(console.error);
  };

  const currentTrackId = usePlayerStore((s) => s.current_track?.track_id ?? null);
  const [addToPlaylistFor, setAddToPlaylistFor] = useState<number[] | null>(null);
  const [editingTrackId, setEditingTrackId] = useState<number | null>(null);

  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: tracks.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 10,
  });
  useScrollMemory(parentRef, !loading && tracks.length > 0);

  return (
    <div className="p-6 h-full flex flex-col">
      <div className="flex items-center justify-between gap-4 mb-4 shrink-0">
        <h1 className="text-2xl font-semibold">Recently Played</h1>
        <div className="flex items-center gap-2">
          <button
            onClick={playAll}
            disabled={!tracks.length}
            className="inline-flex items-center gap-1.5 h-8 px-3 rounded-md bg-primary text-primary-foreground text-sm font-medium disabled:opacity-40 hover:opacity-90 transition-opacity"
          >
            <Play size={13} className="fill-current" />
            Play All
          </button>
          <button
            onClick={shufflePlay}
            disabled={!tracks.length}
            className="inline-flex items-center gap-1.5 h-8 px-3 rounded-md border border-border text-sm font-medium disabled:opacity-40 hover:bg-muted/40 transition-colors"
          >
            <Shuffle size={13} />
            Shuffle Play
          </button>
          <span className="text-sm text-muted-foreground tabular-nums">
            {tracks.length} {tracks.length === 1 ? "track" : "tracks"}
          </span>
        </div>
      </div>

      {error && <p className="text-sm text-destructive mb-3 shrink-0">{error}</p>}

      {loading ? (
        <p className="text-sm text-muted-foreground">Loading…</p>
      ) : tracks.length === 0 ? (
        <p className="text-sm text-muted-foreground">No recently played tracks.</p>
      ) : (
        <div className="rounded-md border border-border overflow-hidden flex-1 min-h-0 flex flex-col">
          <div className={cn("grid items-center bg-muted/40 text-muted-foreground text-sm shrink-0", GRID_COLS)}>
            <div className="px-3 py-2 font-medium">#</div>
            <div />
            <div className="px-3 py-2 font-medium">Title</div>
            <div className="px-3 py-2 font-medium">Artist</div>
            <div className="px-3 py-2 font-medium">Album</div>
            <div className="px-3 py-2 font-medium">Last Played</div>
            <div className="px-3 py-2 font-medium text-right">Duration</div>
            <div className="px-3 py-2 font-medium">Rating</div>
            <div />
          </div>

          <div ref={parentRef} className="flex-1 overflow-auto">
            <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
              {virtualizer.getVirtualItems().map((item) => {
                const track = tracks[item.index];
                const isCurrent = track.id === currentTrackId;
                return (
                  <div
                    key={track.id}
                    style={{
                      position: "absolute",
                      top: 0,
                      left: 0,
                      width: "100%",
                      height: item.size,
                      transform: `translateY(${item.start}px)`,
                    }}
                    onDoubleClick={() => playFromRow(item.index)}
                    className={cn(
                      "grid items-center text-sm border-t border-border hover:bg-muted/30 group cursor-default",
                      GRID_COLS,
                      isCurrent && "bg-muted/40",
                    )}
                  >
                    <div className="px-3 text-muted-foreground tabular-nums">{item.index + 1}</div>
                    <div>
                      <AlbumArt path={track.album_art_path} size={32} rounded="sm" />
                    </div>
                    <div className="px-3 truncate">
                      {track.title ?? track.file_path.split(/[\\/]/).pop()}
                    </div>
                    <div className="px-3 truncate text-muted-foreground">
                      {track.artists.length === 0
                        ? "—"
                        : track.artists.map((a, i) => (
                            <span key={`${a}-${i}`}>
                              {i > 0 && ", "}
                              <Link
                                to={`/songs?artist=${encodeURIComponent(a)}`}
                                className="hover:text-foreground hover:underline"
                              >
                                {a}
                              </Link>
                            </span>
                          ))}
                    </div>
                    <div className="px-3 truncate text-muted-foreground">
                      {track.album_name ? (
                        <Link
                          to={`/songs?album=${encodeURIComponent(track.album_name)}`}
                          className="hover:text-foreground hover:underline"
                        >
                          {track.album_name}
                        </Link>
                      ) : "—"}
                    </div>
                    <div className="px-3 truncate text-muted-foreground tabular-nums">
                      {formatLastPlayed(track.last_played_at)}
                    </div>
                    <div className="px-3 text-right text-muted-foreground tabular-nums">
                      {formatDuration(track.duration_ms)}
                    </div>
                    <div className="px-3">
                      <StarRating value={track.rating} size={12} onChange={(r) => handleRating(track.id, r)} />
                    </div>
                    <div className="px-2 flex items-center justify-end opacity-0 group-hover:opacity-100 transition-opacity">
                      <button
                        onClick={() => addToQueue(track.id).catch(console.error)}
                        className="w-7 h-7 flex items-center justify-center text-muted-foreground hover:text-foreground"
                        aria-label="Add to queue"
                        title="Add to queue"
                      >
                        <ListPlus size={14} />
                      </button>
                      <button
                        onClick={() => setAddToPlaylistFor([track.id])}
                        className="w-7 h-7 flex items-center justify-center text-muted-foreground hover:text-foreground"
                        aria-label="Add to playlist"
                        title="Add to playlist"
                      >
                        <Plus size={14} />
                      </button>
                      <button
                        onClick={() => setEditingTrackId(track.id)}
                        className="w-7 h-7 flex items-center justify-center text-muted-foreground hover:text-foreground"
                        aria-label="Edit metadata"
                        title="Edit metadata"
                      >
                        <Pencil size={13} />
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      )}

      <AddToPlaylistDialog trackIds={addToPlaylistFor} onClose={() => setAddToPlaylistFor(null)} />
      <MetadataEditorDialog
        trackId={editingTrackId}
        onClose={() => setEditingTrackId(null)}
        onSaved={handleMetaSaved}
      />
    </div>
  );
}
