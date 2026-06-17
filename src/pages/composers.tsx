import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { getComposers, type Composer } from "@/lib/ipc";
import { VirtualGrid } from "@/components/virtual-grid";
import { useLibraryRefresh } from "@/lib/library-events";

const GRADIENTS = [
  "from-cyan-500/30 to-blue-500/20",
  "from-violet-500/30 to-purple-500/20",
  "from-amber-500/30 to-orange-500/20",
  "from-emerald-500/30 to-green-500/20",
  "from-rose-500/30 to-pink-500/20",
  "from-sky-500/30 to-indigo-500/20",
];

function gradientFor(name: string): string {
  let hash = 0;
  for (let i = 0; i < name.length; i++) hash = (hash * 31 + name.charCodeAt(i)) >>> 0;
  return GRADIENTS[hash % GRADIENTS.length];
}

export default function Composers() {
  const [composers, setComposers] = useState<Composer[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const refreshTick = useLibraryRefresh();

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getComposers()
      .then((rows) => { if (!cancelled) setComposers(rows); })
      .catch((e) => { if (!cancelled) setError(String(e)); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [refreshTick]);

  return (
    <div className="h-full flex flex-col">
      <div className="px-6 pt-6 pb-4 shrink-0 flex items-baseline justify-between gap-4">
        <h1 className="text-2xl font-semibold">Composers</h1>
        <span className="text-sm text-muted-foreground tabular-nums">
          {composers.length} {composers.length === 1 ? "composer" : "composers"}
        </span>
      </div>

      {error && <p className="text-sm text-destructive px-6 mb-3 shrink-0">{error}</p>}

      <VirtualGrid
        items={composers}
        minItemWidth={180}
        rowHeight={108}
        gap={12}
        px={24}
        paddingBottom={24}
        className="flex-1 min-h-0"
        loading={loading}
        empty={
          <p className="text-sm text-muted-foreground pt-2">
            No composers yet — scan a folder to populate the library.
          </p>
        }
        renderItem={(composer) => (
          <Link
            key={composer.id}
            to={`/songs?composer=${encodeURIComponent(composer.name)}`}
            className={`h-24 overflow-hidden rounded-lg bg-gradient-to-br ${gradientFor(composer.name)} p-4 flex flex-col justify-between border border-border hover:border-foreground/20 transition-colors`}
          >
            <span className="text-base font-semibold truncate">{composer.name}</span>
            <span className="text-xs text-muted-foreground tabular-nums">
              {composer.track_count} {composer.track_count === 1 ? "track" : "tracks"}
            </span>
          </Link>
        )}
      />
    </div>
  );
}
