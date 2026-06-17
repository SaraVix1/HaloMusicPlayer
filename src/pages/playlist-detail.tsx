import { useEffect, useState } from "react";
import { useParams, Link, useNavigate } from "react-router-dom";
import {
  ArrowLeft,
  GripVertical,
  ListMusic,
  Pause,
  Pencil,
  Play,
  Shuffle,
  Trash2,
  X,
} from "lucide-react";
import {
  deletePlaylist,
  getPlaylist,
  removeFromPlaylist,
  renamePlaylist,
  reorderPlaylistTrack,
  setQueueAndPlay,
  setShuffle,
  togglePlayPause,
  type Playlist,
  type PlaylistTrack,
} from "@/lib/ipc";
import { usePlayerStore } from "@/lib/player-store";
import { formatDuration } from "@/lib/format";
import AlbumArt from "@/components/album-art";
import { cn } from "@/lib/utils";

export default function PlaylistDetail() {
  const { id } = useParams<{ id: string }>();
  const playlistId = Number(id);
  const navigate = useNavigate();

  const [playlist, setPlaylist] = useState<Playlist | null>(null);
  const [tracks, setTracks] = useState<PlaylistTrack[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  const [editName, setEditName] = useState("");

  // Drag-to-reorder state (playlist_track_id of the dragged row, hovered index).
  const [dragId, setDragId] = useState<number | null>(null);
  const [overIndex, setOverIndex] = useState<number | null>(null);

  const currentTrackId = usePlayerStore((s) => s.current_track?.track_id ?? null);
  const status = usePlayerStore((s) => s.status);

  const refresh = () => {
    if (!playlistId) return;
    getPlaylist(playlistId)
      .then(([p, ts]) => {
        setPlaylist(p);
        setTracks(ts);
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [playlistId]);

  if (!Number.isFinite(playlistId)) {
    return (
      <div className="p-6">
        <p className="text-sm text-destructive">Invalid playlist id.</p>
      </div>
    );
  }

  const playFrom = (startIndex: number) => {
    if (tracks.length === 0) return;
    setQueueAndPlay(
      tracks.map((t) => t.track_id),
      startIndex,
    ).catch(console.error);
  };

  const playAll = () => {
    if (tracks.length === 0) return;
    setShuffle(false).catch(console.error);
    playFrom(0);
  };

  const shufflePlay = () => {
    if (tracks.length === 0) return;
    const start = Math.floor(Math.random() * tracks.length);
    setShuffle(true).catch(console.error);
    setQueueAndPlay(
      tracks.map((t) => t.track_id),
      start,
    ).catch(console.error);
  };

  const onRowPlay = (idx: number, trackId: number) => {
    if (trackId === currentTrackId) {
      togglePlayPause().catch(console.error);
    } else {
      playFrom(idx);
    }
  };

  const onRename = async () => {
    const name = editName.trim();
    if (!name || !playlist) return;
    try {
      await renamePlaylist(playlist.id, name);
      setEditing(false);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const onDelete = async () => {
    if (!playlist) return;
    if (!confirm(`Delete "${playlist.name}"?`)) return;
    try {
      await deletePlaylist(playlist.id);
      navigate("/playlists");
    } catch (e) {
      setError(String(e));
    }
  };

  const onRemoveTrack = async (playlistTrackId: number) => {
    try {
      await removeFromPlaylist(playlistTrackId);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const onDrop = async (toIndex: number) => {
    const id = dragId;
    setDragId(null);
    setOverIndex(null);
    if (id == null) return;
    const fromIndex = tracks.findIndex((t) => t.playlist_track_id === id);
    if (fromIndex === -1 || fromIndex === toIndex) return;
    // Optimistic reorder for instant feedback; refresh reconciles with the DB.
    const next = [...tracks];
    const [moved] = next.splice(fromIndex, 1);
    next.splice(toIndex, 0, moved);
    setTracks(next);
    try {
      await reorderPlaylistTrack(id, toIndex);
      refresh();
    } catch (e) {
      setError(String(e));
      refresh();
    }
  };

  return (
    <div className="p-6">
      <div className="flex items-center gap-2 mb-3">
        <Link
          to="/playlists"
          className="inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
        >
          <ArrowLeft size={12} />
          Playlists
        </Link>
      </div>

      {loading ? (
        <p className="text-sm text-muted-foreground">Loading…</p>
      ) : !playlist ? (
        <p className="text-sm text-destructive">{error ?? "Not found."}</p>
      ) : (
        <>
          <div className="flex items-start gap-4 mb-6">
            <div className="w-24 h-24 rounded-md bg-muted text-muted-foreground/60 flex items-center justify-center shrink-0">
              <ListMusic size={40} />
            </div>
            <div className="min-w-0 flex-1">
              {editing ? (
                <div className="flex items-center gap-2 mb-1">
                  <input
                    autoFocus
                    value={editName}
                    onChange={(e) => setEditName(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") onRename();
                      if (e.key === "Escape") setEditing(false);
                    }}
                    className="flex-1 bg-transparent border-b border-border outline-none text-xl font-semibold py-1"
                  />
                  <button
                    onClick={onRename}
                    className="rounded-md bg-primary text-primary-foreground px-3 py-1 text-xs"
                  >
                    Save
                  </button>
                  <button
                    onClick={() => setEditing(false)}
                    className="rounded-md border border-border px-3 py-1 text-xs hover:bg-muted/40"
                  >
                    Cancel
                  </button>
                </div>
              ) : (
                <div className="flex items-center gap-2">
                  <h1 className="text-2xl font-semibold truncate">
                    {playlist.name}
                  </h1>
                  <button
                    onClick={() => {
                      setEditing(true);
                      setEditName(playlist.name);
                    }}
                    className="text-muted-foreground hover:text-foreground"
                    aria-label="Rename"
                  >
                    <Pencil size={14} />
                  </button>
                </div>
              )}
              <p className="text-sm text-muted-foreground tabular-nums mt-1">
                {playlist.track_count}{" "}
                {playlist.track_count === 1 ? "track" : "tracks"}
              </p>
              <div className="flex items-center gap-2 mt-3">
                <button
                  onClick={playAll}
                  disabled={tracks.length === 0}
                  className="inline-flex items-center gap-1.5 rounded-md bg-primary text-primary-foreground px-3 py-1.5 text-sm disabled:opacity-50 hover:opacity-90"
                >
                  <Play size={14} />
                  Play
                </button>
                <button
                  onClick={shufflePlay}
                  disabled={tracks.length === 0}
                  className="inline-flex items-center gap-1.5 rounded-md border border-border px-3 py-1.5 text-sm disabled:opacity-50 hover:bg-muted/40"
                >
                  <Shuffle size={14} />
                  Shuffle
                </button>
                <button
                  onClick={onDelete}
                  className="inline-flex items-center gap-1.5 rounded-md border border-border px-3 py-1.5 text-sm hover:bg-destructive/10 hover:text-destructive hover:border-destructive/40"
                >
                  <Trash2 size={14} />
                  Delete
                </button>
              </div>
            </div>
          </div>

          {error && <p className="text-sm text-destructive mb-3">{error}</p>}

          {tracks.length === 0 ? (
            <p className="text-sm text-muted-foreground">
              This playlist is empty. Add songs from{" "}
              <Link to="/songs" className="underline">
                All Songs
              </Link>
              .
            </p>
          ) : (
            <div className="rounded-md border border-border overflow-hidden">
              {tracks.map((row, idx) => {
                const isCurrent = row.track_id === currentTrackId;
                const isPlaying = isCurrent && status === "playing";
                return (
                  <div
                    key={row.playlist_track_id}
                    onDoubleClick={() => playFrom(idx)}
                    onDragOver={(e) => {
                      if (dragId != null) {
                        e.preventDefault();
                        setOverIndex(idx);
                      }
                    }}
                    onDrop={(e) => {
                      e.preventDefault();
                      onDrop(idx);
                    }}
                    className={cn(
                      "flex items-center gap-3 px-3 py-1.5 border-t border-border first:border-t-0 group",
                      isCurrent && "bg-muted/40",
                      dragId === row.playlist_track_id && "opacity-40",
                      overIndex === idx && dragId != null && "border-t-2 border-t-primary",
                    )}
                  >
                    <button
                      draggable
                      onDragStart={(e) => {
                        setDragId(row.playlist_track_id);
                        e.dataTransfer.effectAllowed = "move";
                      }}
                      onDragEnd={() => {
                        setDragId(null);
                        setOverIndex(null);
                      }}
                      className="w-5 h-7 flex items-center justify-center text-muted-foreground/40 hover:text-foreground cursor-grab active:cursor-grabbing opacity-0 group-hover:opacity-100 transition-opacity"
                      aria-label="Drag to reorder"
                    >
                      <GripVertical size={14} />
                    </button>
                    <span className="w-6 text-xs text-muted-foreground tabular-nums text-right">
                      {idx + 1}
                    </span>
                    <button
                      onClick={() => onRowPlay(idx, row.track_id)}
                      className="w-7 h-7 flex items-center justify-center text-muted-foreground hover:text-foreground"
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
                    <div className="flex items-center opacity-0 group-hover:opacity-100 transition-opacity">
                      <button
                        onClick={() => onRemoveTrack(row.playlist_track_id)}
                        className="w-7 h-7 flex items-center justify-center text-muted-foreground hover:text-foreground"
                        aria-label="Remove"
                      >
                        <X size={14} />
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </>
      )}
    </div>
  );
}
