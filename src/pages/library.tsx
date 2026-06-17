import { useEffect, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { getLastTab, setLastTab } from "@/lib/tab-memory";
import {
  getAlbumArtists,
  getArtists,
  getComposers,
  getGenres,
  getYears,
  type AlbumArtist,
  type Artist,
  type Composer,
  type Genre,
  type YearStat,
} from "@/lib/ipc";
import AlbumArt from "@/components/album-art";
import { VirtualGrid } from "@/components/virtual-grid";
import { initials } from "@/lib/format";
import { useLibraryRefresh } from "@/lib/library-events";
import { cn } from "@/lib/utils";

// ── Tabs ──────────────────────────────────────────────────────────────────────

const TABS = [
  { key: "artists",       label: "Artists" },
  { key: "album-artists", label: "Album Artists" },
  { key: "composers",     label: "Composers" },
  { key: "genres",        label: "Genres" },
  { key: "years",         label: "Years" },
] as const;

type TabKey = (typeof TABS)[number]["key"];

// ── Shared gradient helper ────────────────────────────────────────────────────

const GRADIENTS = [
  "from-rose-500/30 to-orange-500/20",
  "from-amber-500/30 to-yellow-500/20",
  "from-emerald-500/30 to-teal-500/20",
  "from-sky-500/30 to-indigo-500/20",
  "from-violet-500/30 to-fuchsia-500/20",
  "from-pink-500/30 to-rose-500/20",
  "from-cyan-500/30 to-blue-500/20",
];

function gradientFor(name: string): string {
  let hash = 0;
  for (let i = 0; i < name.length; i++) hash = (hash * 31 + name.charCodeAt(i)) >>> 0;
  return GRADIENTS[hash % GRADIENTS.length];
}

// ── Avatar helper ─────────────────────────────────────────────────────────────

function Avatar({ name, album_art_path }: { name: string; album_art_path: string | null }) {
  if (album_art_path) {
    return (
      <AlbumArt
        path={album_art_path}
        size={140}
        rounded="full"
        className="w-full aspect-square h-auto"
      />
    );
  }
  return (
    <div className="w-full aspect-square rounded-full bg-muted text-muted-foreground flex items-center justify-center text-3xl font-medium">
      {initials(name) || "?"}
    </div>
  );
}

// ── Tab components ────────────────────────────────────────────────────────────

function ArtistsTab() {
  const [items, setItems] = useState<Artist[]>([]);
  const [loading, setLoading] = useState(true);
  const refreshTick = useLibraryRefresh();

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getArtists()
      .then((r) => { if (!cancelled) { setItems(r); setLoading(false); } })
      .catch(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [refreshTick]);

  return (
    <VirtualGrid
      items={items}
      minItemWidth={140}
      rowHeight={210}
      gap={20}
      px={24}
      paddingTop={8}
      paddingBottom={24}
      className="h-full"
      loading={loading}
      scrollKey="/library?tab=artists"
      empty={<p className="text-sm text-muted-foreground pt-2">No artists yet — scan a folder to get started.</p>}
      renderItem={(a) => (
        <Link
          key={a.id}
          to={`/songs?artist=${encodeURIComponent(a.name)}`}
          className="group flex flex-col items-center gap-2 text-center"
        >
          <div className="w-full group-hover:opacity-90 transition-opacity">
            <Avatar name={a.name} album_art_path={a.album_art_path} />
          </div>
          <div className="min-w-0 w-full">
            <p className="text-sm font-medium truncate">{a.name}</p>
            <p className="text-xs text-muted-foreground tabular-nums">
              {a.track_count} {a.track_count === 1 ? "track" : "tracks"}
            </p>
          </div>
        </Link>
      )}
    />
  );
}

function AlbumArtistsTab() {
  const [items, setItems] = useState<AlbumArtist[]>([]);
  const [loading, setLoading] = useState(true);
  const refreshTick = useLibraryRefresh();

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getAlbumArtists()
      .then((r) => { if (!cancelled) { setItems(r); setLoading(false); } })
      .catch(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [refreshTick]);

  return (
    <VirtualGrid
      items={items}
      minItemWidth={140}
      rowHeight={210}
      gap={20}
      px={24}
      paddingTop={8}
      paddingBottom={24}
      className="h-full"
      loading={loading}
      scrollKey="/library?tab=album-artists"
      empty={<p className="text-sm text-muted-foreground pt-2">No album artists yet — scan a folder to get started.</p>}
      renderItem={(a) => (
        <Link
          key={a.id}
          to={`/songs?album_artist=${encodeURIComponent(a.name)}`}
          className="group flex flex-col items-center gap-2 text-center"
        >
          <div className="w-full group-hover:opacity-90 transition-opacity">
            <Avatar name={a.name} album_art_path={a.album_art_path} />
          </div>
          <div className="min-w-0 w-full">
            <p className="text-sm font-medium truncate">{a.name}</p>
            <p className="text-xs text-muted-foreground tabular-nums">
              {a.album_count} {a.album_count === 1 ? "album" : "albums"}
              {" · "}
              {a.track_count} {a.track_count === 1 ? "track" : "tracks"}
            </p>
          </div>
        </Link>
      )}
    />
  );
}

function ComposersTab() {
  const [items, setItems] = useState<Composer[]>([]);
  const [loading, setLoading] = useState(true);
  const refreshTick = useLibraryRefresh();

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getComposers()
      .then((r) => { if (!cancelled) { setItems(r); setLoading(false); } })
      .catch(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [refreshTick]);

  return (
    <VirtualGrid
      items={items}
      minItemWidth={180}
      rowHeight={108}
      gap={12}
      px={24}
      paddingTop={8}
      paddingBottom={24}
      className="h-full"
      loading={loading}
      scrollKey="/library?tab=composers"
      empty={<p className="text-sm text-muted-foreground pt-2">No composers yet — scan a folder to get started.</p>}
      renderItem={(c) => (
        <Link
          key={c.id}
          to={`/songs?composer=${encodeURIComponent(c.name)}`}
          className={`h-24 overflow-hidden rounded-lg bg-gradient-to-br ${gradientFor(c.name)} p-4 flex flex-col justify-between border border-border hover:border-foreground/20 transition-colors`}
        >
          <span className="text-base font-semibold truncate">{c.name}</span>
          <span className="text-xs text-muted-foreground tabular-nums">
            {c.track_count} {c.track_count === 1 ? "track" : "tracks"}
          </span>
        </Link>
      )}
    />
  );
}

function GenresTab() {
  const [items, setItems] = useState<Genre[]>([]);
  const [loading, setLoading] = useState(true);
  const refreshTick = useLibraryRefresh();

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getGenres()
      .then((r) => { if (!cancelled) { setItems(r); setLoading(false); } })
      .catch(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [refreshTick]);

  return (
    <VirtualGrid
      items={items}
      minItemWidth={180}
      rowHeight={108}
      gap={12}
      px={24}
      paddingTop={8}
      paddingBottom={24}
      className="h-full"
      loading={loading}
      scrollKey="/library?tab=genres"
      empty={<p className="text-sm text-muted-foreground pt-2">No genres yet — scan a folder to get started.</p>}
      renderItem={(g) => (
        <Link
          key={g.id}
          to={`/songs?genre=${encodeURIComponent(g.name)}`}
          className={`h-24 overflow-hidden rounded-lg bg-gradient-to-br ${gradientFor(g.name)} p-4 flex flex-col justify-between border border-border hover:border-foreground/20 transition-colors`}
        >
          <span className="text-base font-semibold truncate">{g.name}</span>
          <span className="text-xs text-muted-foreground tabular-nums">
            {g.track_count} {g.track_count === 1 ? "track" : "tracks"}
          </span>
        </Link>
      )}
    />
  );
}

function YearsTab() {
  const [items, setItems] = useState<YearStat[]>([]);
  const [loading, setLoading] = useState(true);
  const refreshTick = useLibraryRefresh();

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getYears()
      .then((r) => { if (!cancelled) { setItems(r); setLoading(false); } })
      .catch(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [refreshTick]);

  return (
    <VirtualGrid
      items={items}
      minItemWidth={160}
      rowHeight={108}
      gap={12}
      px={24}
      paddingTop={8}
      paddingBottom={24}
      className="h-full"
      loading={loading}
      scrollKey="/library?tab=years"
      empty={<p className="text-sm text-muted-foreground pt-2">No year tags found — scan a folder to get started.</p>}
      renderItem={({ year, track_count }) => (
        <Link
          key={year}
          to={`/songs?year=${year}`}
          className="h-24 rounded-lg border border-border hover:border-foreground/20 transition-colors p-4 flex flex-col justify-between bg-muted/20 hover:bg-muted/40"
        >
          <span className="text-2xl font-semibold tabular-nums">{year}</span>
          <span className="text-xs text-muted-foreground tabular-nums">
            {track_count} {track_count === 1 ? "track" : "tracks"}
          </span>
        </Link>
      )}
    />
  );
}

// ── Tab map ───────────────────────────────────────────────────────────────────

const TAB_CONTENT: Record<TabKey, React.ComponentType> = {
  "artists":       ArtistsTab,
  "album-artists": AlbumArtistsTab,
  "composers":     ComposersTab,
  "genres":        GenresTab,
  "years":         YearsTab,
};

// ── Page ──────────────────────────────────────────────────────────────────────

export default function Library() {
  const [params, setParams] = useSearchParams();
  const remembered = getLastTab("library") as TabKey | null;
  const activeTab =
    (params.get("tab") as TabKey | null) ?? remembered ?? "artists";

  // When arriving via the sidebar (no ?tab=), reflect the remembered tab in the
  // URL so the scroll key matches what was saved for that tab.
  useEffect(() => {
    if (!params.get("tab")) {
      setParams({ tab: activeTab }, { replace: true });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const [visited, setVisited] = useState<Set<TabKey>>(() => new Set([activeTab]));

  const setTab = (key: TabKey) => {
    setVisited((prev) => new Set([...prev, key]));
    setLastTab("library", key);
    setParams({ tab: key }, { replace: true });
  };

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="px-6 pt-6 pb-0 shrink-0">
        <h1 className="text-2xl font-semibold mb-4">Library</h1>
        <div className="flex gap-1 border-b border-border">
          {TABS.map(({ key, label }) => (
            <button
              key={key}
              onClick={() => setTab(key)}
              className={cn(
                "px-3 py-2 text-sm font-medium border-b-2 -mb-px transition-colors",
                activeTab === key
                  ? "border-foreground text-foreground"
                  : "border-transparent text-muted-foreground hover:text-foreground",
              )}
            >
              {label}
            </button>
          ))}
        </div>
      </div>

      {/* Tab panels — all mounted after first visit, hidden when inactive */}
      <div className="flex-1 min-h-0 overflow-hidden">
        {TABS.map(({ key }) => {
          const Content = TAB_CONTENT[key];
          return (
            <div
              key={key}
              className="h-full"
              style={{ display: activeTab === key ? "block" : "none" }}
            >
              {visited.has(key) && <Content />}
            </div>
          );
        })}
      </div>
    </div>
  );
}
