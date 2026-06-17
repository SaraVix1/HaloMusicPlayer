import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { ListMusic, Mic2, Search } from "lucide-react";
import {
  getTracks,
  searchLibrary,
  setQueueAndPlay,
  type SearchResults,
} from "@/lib/ipc";
import { useCommandPalette } from "@/lib/command-palette-store";
import AlbumArt from "@/components/album-art";
import { cn } from "@/lib/utils";

type Row =
  | { type: "track"; id: number; title: string; subtitle: string; art: string | null }
  | { type: "album"; name: string; subtitle: string; art: string | null }
  | { type: "artist"; id: number; name: string; subtitle: string }
  | { type: "playlist"; id: number; name: string; subtitle: string };

function flatten(results: SearchResults | null): Row[] {
  if (!results) return [];
  const rows: Row[] = [];
  for (const t of results.tracks) {
    rows.push({
      type: "track",
      id: t.track_id,
      title: t.title,
      subtitle: [t.artists.join(", "), t.album_name].filter(Boolean).join(" · "),
      art: t.album_art_path,
    });
  }
  for (const a of results.albums) {
    rows.push({
      type: "album",
      name: a.name,
      subtitle: `${a.track_count} ${a.track_count === 1 ? "track" : "tracks"}`,
      art: a.album_art_path,
    });
  }
  for (const ar of results.artists) {
    rows.push({
      type: "artist",
      id: ar.id,
      name: ar.name,
      subtitle: `${ar.track_count} ${ar.track_count === 1 ? "track" : "tracks"}`,
    });
  }
  for (const pl of results.playlists) {
    rows.push({
      type: "playlist",
      id: pl.id,
      name: pl.name,
      subtitle: `${pl.track_count} ${pl.track_count === 1 ? "track" : "tracks"}`,
    });
  }
  return rows;
}

export default function CommandPalette() {
  const open = useCommandPalette((s) => s.open);
  const setOpen = useCommandPalette((s) => s.setOpen);
  const navigate = useNavigate();

  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResults | null>(null);
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) {
      setQuery("");
      setResults(null);
      setActiveIndex(0);
    } else {
      setTimeout(() => inputRef.current?.focus(), 0);
    }
  }, [open]);

  useEffect(() => {
    const handle = setTimeout(() => {
      const trimmed = query.trim();
      if (!trimmed) {
        setResults(null);
        return;
      }
      searchLibrary(trimmed)
        .then((r) => {
          setResults(r);
          setActiveIndex(0);
        })
        .catch(console.error);
    }, 120);
    return () => clearTimeout(handle);
  }, [query]);

  const rows = useMemo(() => flatten(results), [results]);

  useEffect(() => {
    const el = listRef.current?.querySelector<HTMLElement>(
      `[data-row-index="${activeIndex}"]`,
    );
    el?.scrollIntoView({ block: "nearest" });
  }, [activeIndex]);

  const onSelect = async (row: Row) => {
    setOpen(false);
    if (row.type === "track") {
      try {
        await setQueueAndPlay([row.id], 0);
      } catch (e) {
        console.error(e);
      }
    } else if (row.type === "album") {
      try {
        const tracks = await getTracks({
          album: row.name,
          sort: "track_number",
          direction: "asc",
        });
        if (tracks.length > 0) {
          await setQueueAndPlay(
            tracks.map((t) => t.id),
            0,
          );
        }
      } catch (e) {
        console.error(e);
      }
      navigate(`/songs?album=${encodeURIComponent(row.name)}&sort=track_number&dir=asc`);
    } else if (row.type === "artist") {
      navigate(`/songs?artist=${encodeURIComponent(row.name)}`);
    } else if (row.type === "playlist") {
      navigate(`/playlists/${row.id}`);
    }
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      setOpen(false);
      return;
    }
    if (rows.length === 0) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActiveIndex((i) => Math.min(i + 1, rows.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActiveIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const row = rows[activeIndex];
      if (row) onSelect(row);
    }
  };

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center pt-[10vh] bg-black/40 backdrop-blur-sm"
      onClick={() => setOpen(false)}
    >
      <div
        className="w-[640px] max-w-[90vw] rounded-lg bg-background border border-border shadow-2xl overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2 px-3 py-2 border-b border-border">
          <Search size={16} className="text-muted-foreground shrink-0" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={onKeyDown}
            placeholder="Search tracks, albums, artists, playlists…"
            className="flex-1 bg-transparent outline-none text-sm py-1.5 placeholder:text-muted-foreground"
          />
          <kbd className="text-[10px] text-muted-foreground border border-border rounded px-1.5 py-0.5">
            Esc
          </kbd>
        </div>

        <div
          ref={listRef}
          className="max-h-[60vh] overflow-y-auto py-1"
        >
          {!results ? (
            <p className="px-3 py-6 text-sm text-muted-foreground text-center">
              Start typing to search.
            </p>
          ) : rows.length === 0 ? (
            <p className="px-3 py-6 text-sm text-muted-foreground text-center">
              No results for "{query}".
            </p>
          ) : (
            <RowList rows={rows} activeIndex={activeIndex} onSelect={onSelect} setActiveIndex={setActiveIndex} />
          )}
        </div>
      </div>
    </div>
  );
}

function RowList({
  rows,
  activeIndex,
  onSelect,
  setActiveIndex,
}: {
  rows: Row[];
  activeIndex: number;
  onSelect: (row: Row) => void;
  setActiveIndex: (i: number) => void;
}) {
  let lastType: Row["type"] | null = null;
  const labels: Record<Row["type"], string> = {
    track: "Songs",
    album: "Albums",
    artist: "Artists",
    playlist: "Playlists",
  };
  return (
    <>
      {rows.map((row, i) => {
        const showHeader = row.type !== lastType;
        lastType = row.type;
        return (
          <div key={`${row.type}-${i}`}>
            {showHeader && (
              <p className="px-3 pt-2 pb-1 text-[10px] uppercase tracking-wider text-muted-foreground">
                {labels[row.type]}
              </p>
            )}
            <button
              data-row-index={i}
              onClick={() => onSelect(row)}
              onMouseEnter={() => setActiveIndex(i)}
              className={cn(
                "w-full flex items-center gap-3 px-3 py-1.5 text-left text-sm",
                i === activeIndex ? "bg-muted/60" : "hover:bg-muted/30",
              )}
            >
              <RowIcon row={row} />
              <div className="min-w-0 flex-1">
                <p className="truncate">{getRowTitle(row)}</p>
                <p className="text-xs text-muted-foreground truncate">
                  {row.subtitle || "—"}
                </p>
              </div>
            </button>
          </div>
        );
      })}
    </>
  );
}

function RowIcon({ row }: { row: Row }) {
  if (row.type === "track" || row.type === "album") {
    return <AlbumArt path={row.art} size={32} rounded="sm" />;
  }
  return (
    <div className="w-8 h-8 rounded-sm bg-muted text-muted-foreground/60 flex items-center justify-center shrink-0">
      {row.type === "artist" ? <Mic2 size={14} /> : <ListMusic size={14} />}
    </div>
  );
}

function getRowTitle(row: Row): string {
  if (row.type === "track") return row.title;
  return row.name;
}
