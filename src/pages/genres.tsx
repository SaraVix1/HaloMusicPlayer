import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { getGenres, type Genre } from "@/lib/ipc";
import { VirtualGrid } from "@/components/virtual-grid";
import { useLibraryRefresh } from "@/lib/library-events";

const GRADIENTS = [
  "from-rose-500/30 to-orange-500/20",
  "from-amber-500/30 to-yellow-500/20",
  "from-emerald-500/30 to-teal-500/20",
  "from-sky-500/30 to-indigo-500/20",
  "from-violet-500/30 to-fuchsia-500/20",
  "from-pink-500/30 to-rose-500/20",
];

function gradientFor(name: string): string {
  let hash = 0;
  for (let i = 0; i < name.length; i++) hash = (hash * 31 + name.charCodeAt(i)) >>> 0;
  return GRADIENTS[hash % GRADIENTS.length];
}

export default function Genres() {
  const [genres, setGenres] = useState<Genre[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const refreshTick = useLibraryRefresh();

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getGenres()
      .then((rows) => { if (!cancelled) setGenres(rows); })
      .catch((e) => { if (!cancelled) setError(String(e)); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [refreshTick]);

  return (
    <div className="h-full flex flex-col">
      <div className="px-6 pt-6 pb-4 shrink-0 flex items-baseline justify-between gap-4">
        <h1 className="text-2xl font-semibold">Genres</h1>
        <span className="text-sm text-muted-foreground tabular-nums">
          {genres.length} {genres.length === 1 ? "genre" : "genres"}
        </span>
      </div>

      {error && <p className="text-sm text-destructive px-6 mb-3 shrink-0">{error}</p>}

      <VirtualGrid
        items={genres}
        minItemWidth={180}
        rowHeight={108}
        gap={12}
        px={24}
        paddingBottom={24}
        className="flex-1 min-h-0"
        loading={loading}
        empty={
          <p className="text-sm text-muted-foreground pt-2">
            No genres yet — scan a folder to populate the library.
          </p>
        }
        renderItem={(genre) => (
          <Link
            key={genre.id}
            to={`/songs?genre=${encodeURIComponent(genre.name)}`}
            className={`h-24 overflow-hidden rounded-lg bg-gradient-to-br ${gradientFor(genre.name)} p-4 flex flex-col justify-between border border-border hover:border-foreground/20 transition-colors`}
          >
            <span className="text-base font-semibold truncate">{genre.name}</span>
            <span className="text-xs text-muted-foreground tabular-nums">
              {genre.track_count} {genre.track_count === 1 ? "track" : "tracks"}
            </span>
          </Link>
        )}
      />
    </div>
  );
}
