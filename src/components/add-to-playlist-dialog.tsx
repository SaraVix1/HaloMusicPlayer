import { useEffect, useState } from "react";
import { ListMusic, Plus } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  addToPlaylist,
  createPlaylist,
  getPlaylists,
  type Playlist,
} from "@/lib/ipc";

interface Props {
  trackIds: number[] | null;
  onClose: () => void;
}

export default function AddToPlaylistDialog({ trackIds, onClose }: Props) {
  const [playlists, setPlaylists] = useState<Playlist[]>([]);
  const [loading, setLoading] = useState(false);
  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState("");
  const [error, setError] = useState<string | null>(null);

  const open = trackIds !== null;

  useEffect(() => {
    if (!open) return;
    setLoading(true);
    setError(null);
    setCreating(false);
    setNewName("");
    getPlaylists()
      .then(setPlaylists)
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [open]);

  const addAndClose = async (playlistId: number) => {
    if (!trackIds) return;
    try {
      await addToPlaylist(playlistId, trackIds);
      onClose();
    } catch (e) {
      setError(String(e));
    }
  };

  const onCreate = async () => {
    const name = newName.trim();
    if (!name || !trackIds) return;
    try {
      const created = await createPlaylist(name);
      await addToPlaylist(created.id, trackIds);
      onClose();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Add to playlist</DialogTitle>
        </DialogHeader>

        {error && <p className="text-sm text-destructive">{error}</p>}

        <div className="max-h-72 overflow-y-auto -mx-1">
          {loading ? (
            <p className="text-sm text-muted-foreground px-1 py-3">Loading…</p>
          ) : playlists.length === 0 ? (
            <p className="text-sm text-muted-foreground px-1 py-3">
              No playlists yet — create one below.
            </p>
          ) : (
            playlists.map((p) => (
              <button
                key={p.id}
                onClick={() => addAndClose(p.id)}
                className="w-full flex items-center gap-2 px-2 py-1.5 rounded-md text-left text-sm hover:bg-muted/40"
              >
                <ListMusic size={14} className="text-muted-foreground shrink-0" />
                <span className="truncate flex-1">{p.name}</span>
                <span className="text-xs text-muted-foreground tabular-nums">
                  {p.track_count}
                </span>
              </button>
            ))
          )}
        </div>

        <div className="border-t border-border pt-3">
          {creating ? (
            <div className="flex items-center gap-2">
              <input
                autoFocus
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") onCreate();
                  if (e.key === "Escape") setCreating(false);
                }}
                placeholder="Playlist name"
                className="flex-1 bg-transparent border-b border-border outline-none text-sm py-1"
              />
              <button
                onClick={onCreate}
                disabled={!newName.trim()}
                className="rounded-md bg-primary text-primary-foreground px-3 py-1 text-xs disabled:opacity-50"
              >
                Create + Add
              </button>
              <button
                onClick={() => setCreating(false)}
                className="rounded-md border border-border px-3 py-1 text-xs hover:bg-muted/40"
              >
                Cancel
              </button>
            </div>
          ) : (
            <button
              onClick={() => setCreating(true)}
              className="w-full flex items-center gap-2 px-2 py-1.5 rounded-md text-left text-sm hover:bg-muted/40 text-muted-foreground hover:text-foreground"
            >
              <Plus size={14} />
              New playlist…
            </button>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
