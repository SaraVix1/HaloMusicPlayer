import { useEffect, useMemo, useState } from "react";
import { Folder, Music, Play, Shuffle } from "lucide-react";
import { getFolderTracks, setQueueAndPlay, setShuffle, type FolderTrack } from "@/lib/ipc";
import { formatDuration } from "@/lib/format";
import { useLibraryRefresh } from "@/lib/library-events";
import { cn } from "@/lib/utils";
import AlbumArt from "@/components/album-art";

interface TreeNode {
  name: string;
  path: string;
  children: Map<string, TreeNode>;
  tracks: FolderTrack[];
}

function newNode(name: string, path: string): TreeNode {
  return { name, path, children: new Map(), tracks: [] };
}

function relativeSegments(filePath: string, folderPath: string): string[] {
  const norm = (s: string) => s.replace(/\\/g, "/").replace(/\/$/, "");
  const f = norm(folderPath);
  const p = norm(filePath);
  const prefix = p.toLowerCase().startsWith(f.toLowerCase() + "/")
    ? p.slice(f.length + 1)
    : p;
  return prefix.split("/").filter(Boolean);
}

function buildTree(rows: FolderTrack[]): TreeNode[] {
  const roots = new Map<number, TreeNode>();
  for (const row of rows) {
    let root = roots.get(row.folder_id);
    if (!root) {
      root = newNode(row.folder_path, row.folder_path);
      roots.set(row.folder_id, root);
    }
    const segments = relativeSegments(row.file_path, row.folder_path);
    if (segments.length === 0) continue;
    const dirSegments = segments.slice(0, -1);
    let node = root;
    let path = root.path;
    for (const seg of dirSegments) {
      path = `${path}/${seg}`;
      let child = node.children.get(seg);
      if (!child) {
        child = newNode(seg, path);
        node.children.set(seg, child);
      }
      node = child;
    }
    node.tracks.push(row);
  }
  return Array.from(roots.values());
}

// Collect leaf folders (folders that directly contain tracks) sorted by path.
function collectLeaves(nodes: TreeNode[]): TreeNode[] {
  const leaves: TreeNode[] = [];
  for (const n of nodes) {
    if (n.children.size === 0) {
      if (n.tracks.length > 0) leaves.push(n);
    } else {
      leaves.push(
        ...collectLeaves(
          Array.from(n.children.values()).sort((a, b) =>
            a.name.localeCompare(b.name, undefined, { sensitivity: "base" }),
          ),
        ),
      );
    }
  }
  return leaves;
}

function leafName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

// ---------------------------------------------------------------------------
// Right panel
// ---------------------------------------------------------------------------

function FolderDetail({ node }: { node: TreeNode }) {
  const tracks = [...node.tracks].sort((a, b) =>
    a.file_path.localeCompare(b.file_path, undefined, { sensitivity: "base" }),
  );
  const totalDuration = tracks.reduce((s, t) => s + (t.duration_ms ?? 0), 0);
  const artPath = tracks.find((t) => t.album_art_path)?.album_art_path ?? null;

  const playFrom = (index: number) => {
    setShuffle(false).catch(console.error);
    setQueueAndPlay(tracks.map((t) => t.id), index).catch(console.error);
  };

  const shuffleAll = () => {
    setShuffle(true).catch(console.error);
    setQueueAndPlay(tracks.map((t) => t.id), 0).catch(console.error);
  };

  return (
    <div className="flex flex-col h-full min-h-0">
      {/* Header */}
      <div className="flex items-center gap-4 px-6 py-4 border-b border-border shrink-0">
        <AlbumArt path={artPath} className="w-16 h-16 rounded-md shrink-0" />
        <div className="flex-1 min-w-0">
          <h2 className="text-base font-semibold truncate">{leafName(node.path)}</h2>
          <p className="text-xs text-muted-foreground mt-0.5">
            {tracks.length} {tracks.length === 1 ? "track" : "tracks"} ·{" "}
            {formatDuration(totalDuration)}
          </p>
          <div className="flex gap-2 mt-2.5">
            <button
              onClick={() => playFrom(0)}
              className="flex items-center gap-1.5 text-xs bg-primary text-primary-foreground rounded-md px-3 py-1.5 hover:opacity-90 transition-opacity"
            >
              <Play size={11} fill="currentColor" />
              Play All
            </button>
            <button
              onClick={shuffleAll}
              className="flex items-center gap-1.5 text-xs border border-border rounded-md px-3 py-1.5 hover:bg-muted/40 transition-colors"
            >
              <Shuffle size={11} />
              Shuffle
            </button>
          </div>
        </div>
      </div>

      {/* Track list */}
      <div className="flex-1 overflow-y-auto">
        {tracks.map((track, i) => (
          <div
            key={track.id}
            onClick={() => playFrom(i)}
            className="flex items-center gap-3 px-6 py-2 hover:bg-muted/30 cursor-pointer"
          >
            <span className="w-5 text-right text-xs text-muted-foreground tabular-nums shrink-0">
              {i + 1}
            </span>
            <Music size={13} className="text-muted-foreground shrink-0" />
            <span className="flex-1 truncate text-sm">
              {track.title ?? track.file_path.split(/[\\/]/).pop() ?? track.file_path}
            </span>
            <span className="text-xs text-muted-foreground tabular-nums shrink-0">
              {formatDuration(track.duration_ms)}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Page
// ---------------------------------------------------------------------------

export default function FoldersView() {
  const [rows, setRows] = useState<FolderTrack[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<TreeNode | null>(null);
  const refreshTick = useLibraryRefresh();

  useEffect(() => {
    let cancelled = false;
    getFolderTracks()
      .then((r) => { if (!cancelled) setRows(r); })
      .catch((e) => { if (!cancelled) setError(String(e)); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [refreshTick]);

  const tree = useMemo(() => buildTree(rows), [rows]);
  const leaves = useMemo(() => collectLeaves(tree), [tree]);

  // Auto-select the first leaf folder on load.
  useEffect(() => {
    setSelected((prev) => prev ?? leaves[0] ?? null);
  }, [leaves]);

  return (
    <div className="flex h-full min-h-0">
      {/* Left: flat folder list */}
      <div className="w-72 shrink-0 border-r border-border flex flex-col min-h-0">
        <div className="px-4 py-3 border-b border-border shrink-0 flex items-baseline justify-between">
          <span className="text-sm font-semibold">Folders</span>
          {leaves.length > 0 && (
            <span className="text-xs text-muted-foreground tabular-nums">
              {leaves.length} {leaves.length === 1 ? "folder" : "folders"}
            </span>
          )}
        </div>

        <div className="flex-1 overflow-y-auto">
          {loading ? (
            <p className="text-xs text-muted-foreground p-4">Loading…</p>
          ) : error ? (
            <p className="text-xs text-destructive p-4">{error}</p>
          ) : leaves.length === 0 ? (
            <p className="text-xs text-muted-foreground p-4">
              No managed folders yet — add one in Settings.
            </p>
          ) : (
            leaves.map((node) => (
              <button
                key={node.path}
                onClick={() => setSelected(node)}
                className={cn(
                  "w-full flex items-center gap-2.5 px-3 py-2.5 text-left",
                  selected?.path === node.path
                    ? "bg-accent text-accent-foreground"
                    : "hover:bg-muted/40",
                )}
              >
                <Folder
                  size={15}
                  className={cn(
                    "shrink-0",
                    selected?.path === node.path
                      ? "text-accent-foreground"
                      : "text-muted-foreground",
                  )}
                />
                <span className="flex-1 min-w-0">
                  <span className="block truncate text-sm font-medium">
                    {leafName(node.path)}
                  </span>
                  <span
                    className={cn(
                      "block truncate text-xs mt-0.5",
                      selected?.path === node.path
                        ? "text-accent-foreground/60"
                        : "text-muted-foreground",
                    )}
                  >
                    {node.tracks.length} {node.tracks.length === 1 ? "track" : "tracks"}
                  </span>
                </span>
              </button>
            ))
          )}
        </div>
      </div>

      {/* Right: tracks for selected folder */}
      <div className="flex-1 min-w-0 min-h-0">
        {selected ? (
          <FolderDetail node={selected} />
        ) : (
          <div className="flex items-center justify-center h-full text-sm text-muted-foreground">
            Select a folder to view its tracks.
          </div>
        )}
      </div>
    </div>
  );
}
