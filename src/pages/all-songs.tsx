import { useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams, Link } from "react-router-dom";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowDown, ArrowUp, ListPlus, Pencil, Play, Plus, Shuffle, X } from "lucide-react";
import {
  addToQueue,
  getTracks,
  setQueueAndPlay,
  setRating,
  setShuffle,
  type Track,
  type TracksQuery,
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

type SortKey =
  | "title"
  | "artist"
  | "album"
  | "genre"
  | "duration"
  | "scanned_at"
  | "rating"
  | "play_count";

const COLUMNS: { key: SortKey; label: string; className?: string }[] = [
  { key: "title", label: "Title" },
  { key: "artist", label: "Artist" },
  { key: "album", label: "Album" },
  { key: "genre", label: "Genre" },
  { key: "duration", label: "Duration", className: "text-right" },
  { key: "rating", label: "Rating" },
];

const ROW_HEIGHT = 44;

// Grid template shared by header + rows so columns line up.
const GRID_COLS =
  "grid-cols-[48px_48px_minmax(0,2fr)_minmax(0,1.5fr)_minmax(0,1.5fr)_minmax(0,1fr)_80px_100px_72px]";

export default function AllSongs() {
  const [params, setParams] = useSearchParams();
  const [tracks, setTracks] = useState<Track[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const refreshTick = useLibraryRefresh();

  const album = params.get("album") ?? undefined;
  const artist = params.get("artist") ?? undefined;
  const album_artist = params.get("album_artist") ?? undefined;
  const composer = params.get("composer") ?? undefined;
  const genre = params.get("genre") ?? undefined;
  const yearParam = params.get("year");
  const year = yearParam ? parseInt(yearParam, 10) : undefined;
  const sort = (params.get("sort") as SortKey | null) ?? "title";
  const direction = (params.get("dir") as "asc" | "desc" | null) ?? "asc";

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    const query: TracksQuery = { sort, direction };
    if (album) query.album = album;
    if (artist) query.artist = artist;
    if (album_artist) query.album_artist = album_artist;
    if (composer) query.composer = composer;
    if (genre) query.genre = genre;
    if (year) query.year = year;
    getTracks(query)
      .then((rows) => {
        if (!cancelled) setTracks(rows);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [album, artist, album_artist, composer, genre, year, sort, direction, refreshTick]);

  const filters = useMemo(() => {
    const out: { key: string; label: string }[] = [];
    if (album) out.push({ key: "album", label: `Album: ${album}` });
    if (artist) out.push({ key: "artist", label: `Artist: ${artist}` });
    if (album_artist) out.push({ key: "album_artist", label: `Album Artist: ${album_artist}` });
    if (composer) out.push({ key: "composer", label: `Composer: ${composer}` });
    if (genre) out.push({ key: "genre", label: `Genre: ${genre}` });
    if (year) out.push({ key: "year", label: `Year: ${year}` });
    return out;
  }, [album, artist, album_artist, composer, genre, year]);

  const handleSort = (key: SortKey) => {
    const next = new URLSearchParams(params);
    if (sort === key) {
      next.set("dir", direction === "asc" ? "desc" : "asc");
    } else {
      next.set("sort", key);
      next.set("dir", "asc");
    }
    setParams(next, { replace: true });
  };

  const clearFilter = (key: string) => {
    const next = new URLSearchParams(params);
    next.delete(key);
    setParams(next, { replace: true });
  };

  const playFromRow = (startIndex: number) => {
    if (tracks.length === 0) return;
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
    setTracks((prev) =>
      prev.map((t) => (t.id === trackId ? { ...t, rating } : t)),
    );
    setRating(trackId, rating).catch(console.error);
  };

  const handleMetaSaved = (_savedId: number) => {
    const query: TracksQuery = { sort, direction };
    if (album) query.album = album;
    if (artist) query.artist = artist;
    if (album_artist) query.album_artist = album_artist;
    if (composer) query.composer = composer;
    if (genre) query.genre = genre;
    if (year) query.year = year;
    getTracks(query).then(setTracks).catch(console.error);
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
        <h1 className="text-2xl font-semibold">All Songs</h1>
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

      {filters.length > 0 && (
        <div className="flex flex-wrap gap-2 mb-4 shrink-0">
          {filters.map((f) => (
            <button
              key={f.key}
              onClick={() => clearFilter(f.key)}
              className="inline-flex items-center gap-1 rounded-full bg-muted px-2.5 py-1 text-xs font-medium hover:bg-muted/70"
            >
              {f.label}
              <X size={12} />
            </button>
          ))}
        </div>
      )}

      {error && <p className="text-sm text-destructive mb-3 shrink-0">{error}</p>}

      {loading ? (
        <p className="text-sm text-muted-foreground">Loading…</p>
      ) : tracks.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          No tracks found.{" "}
          <Link to="/settings" className="underline">
            Add a folder and scan
          </Link>{" "}
          to get started.
        </p>
      ) : (
        <div className="rounded-md border border-border overflow-hidden flex-1 min-h-0 flex flex-col">
          <div
            className={cn(
              "grid items-center bg-muted/40 text-muted-foreground text-sm shrink-0",
              GRID_COLS,
            )}
          >
            <div className="px-3 py-2 font-medium">#</div>
            <div></div>
            {COLUMNS.map((col) => (
              <button
                key={col.key}
                className={cn(
                  "px-3 py-2 font-medium text-left hover:text-foreground transition-colors",
                  col.className,
                )}
                onClick={() => handleSort(col.key)}
              >
                <span className="inline-flex items-center gap-1">
                  {col.label}
                  {sort === col.key &&
                    (direction === "asc" ? (
                      <ArrowUp size={12} />
                    ) : (
                      <ArrowDown size={12} />
                    ))}
                </span>
              </button>
            ))}
            <div></div>
          </div>

          <div ref={parentRef} className="flex-1 overflow-auto">
            <div
              style={{ height: virtualizer.getTotalSize(), position: "relative" }}
            >
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
                    <div className="px-3 text-muted-foreground tabular-nums">
                      {item.index + 1}
                    </div>
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
                      ) : (
                        "—"
                      )}
                    </div>
                    <div className="px-3 truncate text-muted-foreground">
                      {track.genres.length === 0
                        ? "—"
                        : track.genres.map((g, i) => (
                            <span key={`${g}-${i}`}>
                              {i > 0 && ", "}
                              <Link
                                to={`/songs?genre=${encodeURIComponent(g)}`}
                                className="hover:text-foreground hover:underline"
                              >
                                {g}
                              </Link>
                            </span>
                          ))}
                    </div>
                    <div className="px-3 text-right text-muted-foreground tabular-nums">
                      {formatDuration(track.duration_ms)}
                    </div>
                    <div className="px-3">
                      <StarRating
                        value={track.rating}
                        size={12}
                        onChange={(r) => handleRating(track.id, r)}
                      />
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

      <AddToPlaylistDialog
        trackIds={addToPlaylistFor}
        onClose={() => setAddToPlaylistFor(null)}
      />
      <MetadataEditorDialog
        trackId={editingTrackId}
        onClose={() => setEditingTrackId(null)}
        onSaved={handleMetaSaved}
      />
    </div>
  );
}
