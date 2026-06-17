import { useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import {
  ListMusic,
  Plus,
  Lock,
  Sparkles,
  CalendarPlus,
  TrendingUp,
  Star,
  VolumeX,
  History,
  ChevronDown,
  type LucideIcon,
} from "lucide-react";
import {
  createPlaylist,
  getPlaylists,
  createSmartPlaylist,
  getSmartPlaylists,
  type Playlist,
  type SmartPlaylist,
} from "@/lib/ipc";
import { useLibraryRefresh } from "@/lib/library-events";
import { cn } from "@/lib/utils";

// ── Built-in smart playlist presentation ────────────────────────────────────

const BUILTIN_ICONS: Record<string, LucideIcon> = {
  "Recently Added": CalendarPlus,
  "Most Played": TrendingUp,
  "Top Rated": Star,
  "Never Played": VolumeX,
  "Recently Played": History,
};

const BUILTIN_DESCRIPTIONS: Record<string, string> = {
  "Recently Added": "Added in the last 30 days",
  "Most Played": "Top 50 by play count",
  "Top Rated": "Rated 4 stars or above",
  "Never Played": "Never been played",
  "Recently Played": "Played in the last 7 days",
};

// IDs 1-5 are the built-in smart playlists created by the migration.
const BUILTIN_IDS = new Set([1, 2, 3, 4, 5]);

// ── Unified item model ──────────────────────────────────────────────────────

type UnifiedItem =
  | { kind: "manual"; id: number; name: string; trackCount: number }
  | { kind: "smart"; id: number; name: string; matchMode: SmartPlaylist["match_mode"]; isBuiltin: boolean };

type Filter = "all" | "manual" | "smart";

function smartSubtitle(name: string, matchMode: SmartPlaylist["match_mode"]): string {
  if (BUILTIN_DESCRIPTIONS[name]) return BUILTIN_DESCRIPTIONS[name];
  return matchMode === "all" ? "Matches all rules" : "Matches any rule";
}

// ── Card ────────────────────────────────────────────────────────────────────

function PlaylistCard({ item }: { item: UnifiedItem }) {
  const to = item.kind === "manual" ? `/playlists/${item.id}` : `/smart-playlists/${item.id}`;
  const Icon: LucideIcon =
    item.kind === "manual" ? ListMusic : (BUILTIN_ICONS[item.name] ?? Sparkles);
  const subtitle =
    item.kind === "manual"
      ? `${item.trackCount} ${item.trackCount === 1 ? "track" : "tracks"}`
      : smartSubtitle(item.name, item.matchMode);

  return (
    <Link
      to={to}
      className="group flex items-center gap-3 rounded-lg border border-border p-3 hover:bg-muted/30 transition-colors"
    >
      <div className="w-12 h-12 rounded-md bg-muted text-muted-foreground/60 flex items-center justify-center shrink-0">
        <Icon size={20} />
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          <p className="text-sm font-medium truncate">{item.name}</p>
          {item.kind === "smart" && (
            <Sparkles size={11} className="text-primary/70 shrink-0" aria-label="Smart playlist" />
          )}
        </div>
        <p className="text-xs text-muted-foreground tabular-nums truncate">{subtitle}</p>
      </div>
      {item.kind === "smart" && item.isBuiltin && (
        <Lock size={12} className="text-muted-foreground/40 shrink-0" />
      )}
    </Link>
  );
}

// ── Page ────────────────────────────────────────────────────────────────────

export default function Playlists() {
  const navigate = useNavigate();
  const [manual, setManual] = useState<Playlist[]>([]);
  const [smart, setSmart] = useState<SmartPlaylist[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState<Filter>("all");

  // New-playlist menu + inline creation.
  const [menuOpen, setMenuOpen] = useState(false);
  const [creatingKind, setCreatingKind] = useState<"manual" | "smart" | null>(null);
  const [newName, setNewName] = useState("");
  const menuRef = useRef<HTMLDivElement>(null);

  const refreshTick = useLibraryRefresh();

  const refresh = () => {
    setLoading(true);
    Promise.all([getPlaylists(), getSmartPlaylists()])
      .then(([m, s]) => {
        setManual(m);
        setSmart(s);
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    refresh();
  }, [refreshTick]);

  // Close the New menu on outside click.
  useEffect(() => {
    if (!menuOpen) return;
    const onClick = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) setMenuOpen(false);
    };
    window.addEventListener("mousedown", onClick);
    return () => window.removeEventListener("mousedown", onClick);
  }, [menuOpen]);

  const items = useMemo<UnifiedItem[]>(() => {
    const m: UnifiedItem[] = manual.map((p) => ({
      kind: "manual",
      id: p.id,
      name: p.name,
      trackCount: p.track_count,
    }));
    const s: UnifiedItem[] = smart.map((p) => ({
      kind: "smart",
      id: p.id,
      name: p.name,
      matchMode: p.match_mode,
      isBuiltin: BUILTIN_IDS.has(p.id),
    }));
    if (filter === "manual") return m;
    if (filter === "smart") return s;
    return [...m, ...s];
  }, [manual, smart, filter]);

  const startCreate = (kind: "manual" | "smart") => {
    setMenuOpen(false);
    setCreatingKind(kind);
    setNewName("");
  };

  const onCreate = async () => {
    const name = newName.trim();
    if (!name || !creatingKind) return;
    try {
      if (creatingKind === "manual") {
        const p = await createPlaylist(name);
        navigate(`/playlists/${p.id}`);
      } else {
        const p = await createSmartPlaylist(name);
        navigate(`/smart-playlists/${p.id}`);
      }
    } catch (e) {
      setError(String(e));
    }
  };

  const totalCount = manual.length + smart.length;

  return (
    <div className="p-6">
      <div className="flex items-baseline justify-between gap-4 mb-4">
        <h1 className="text-2xl font-semibold">Playlists</h1>
        <div className="flex items-center gap-3">
          <span className="text-sm text-muted-foreground tabular-nums">
            {totalCount} {totalCount === 1 ? "playlist" : "playlists"}
          </span>
          <div className="relative" ref={menuRef}>
            <button
              onClick={() => setMenuOpen((v) => !v)}
              className="inline-flex items-center gap-1.5 rounded-md bg-primary text-primary-foreground px-2.5 py-1 text-xs hover:opacity-90"
            >
              <Plus size={14} />
              New
              <ChevronDown size={12} />
            </button>
            {menuOpen && (
              <div className="absolute right-0 top-full mt-1 z-10 w-44 rounded-md border border-border bg-popover shadow-md py-1">
                <button
                  onClick={() => startCreate("manual")}
                  className="w-full flex items-center gap-2 px-3 py-1.5 text-xs text-left hover:bg-muted/50"
                >
                  <ListMusic size={14} />
                  New playlist
                </button>
                <button
                  onClick={() => startCreate("smart")}
                  className="w-full flex items-center gap-2 px-3 py-1.5 text-xs text-left hover:bg-muted/50"
                >
                  <Sparkles size={14} />
                  New smart playlist
                </button>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Filter */}
      <div className="flex items-center gap-1 mb-4">
        {(["all", "manual", "smart"] as const).map((f) => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            className={cn(
              "px-2.5 py-1 rounded-md text-xs transition-colors capitalize",
              filter === f
                ? "bg-secondary text-secondary-foreground"
                : "text-muted-foreground hover:bg-muted/40",
            )}
          >
            {f}
          </button>
        ))}
      </div>

      {/* Inline create */}
      {creatingKind && (
        <div className="mb-4 flex items-center gap-2 rounded-md border border-border p-3">
          {creatingKind === "smart" ? <Sparkles size={16} /> : <ListMusic size={16} />}
          <input
            autoFocus
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") onCreate();
              if (e.key === "Escape") setCreatingKind(null);
            }}
            placeholder={creatingKind === "smart" ? "Smart playlist name" : "Playlist name"}
            className="flex-1 bg-transparent border-b border-border outline-none text-sm py-1"
          />
          <button
            onClick={onCreate}
            disabled={!newName.trim()}
            className="rounded-md bg-primary text-primary-foreground px-3 py-1 text-xs disabled:opacity-50"
          >
            Create
          </button>
          <button
            onClick={() => setCreatingKind(null)}
            className="rounded-md border border-border px-3 py-1 text-xs hover:bg-muted/40"
          >
            Cancel
          </button>
        </div>
      )}

      {error && <p className="text-sm text-destructive mb-3">{error}</p>}

      {loading ? (
        <p className="text-sm text-muted-foreground">Loading…</p>
      ) : items.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          {filter === "smart"
            ? "No smart playlists yet."
            : filter === "manual"
              ? "No playlists yet. Create one to organize tracks."
              : "No playlists yet. Create one to organize tracks."}
        </p>
      ) : (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-3">
          {items.map((item) => (
            <PlaylistCard key={`${item.kind}-${item.id}`} item={item} />
          ))}
        </div>
      )}
    </div>
  );
}
