import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { getYears, type YearStat } from "@/lib/ipc";
import { VirtualGrid } from "@/components/virtual-grid";
import { useLibraryRefresh } from "@/lib/library-events";

export default function Years() {
  const [years, setYears] = useState<YearStat[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const refreshTick = useLibraryRefresh();

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getYears()
      .then((rows) => { if (!cancelled) setYears(rows); })
      .catch((e) => { if (!cancelled) setError(String(e)); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [refreshTick]);

  return (
    <div className="h-full flex flex-col">
      <div className="px-6 pt-6 pb-4 shrink-0 flex items-baseline justify-between gap-4">
        <h1 className="text-2xl font-semibold">Years</h1>
        <span className="text-sm text-muted-foreground tabular-nums">
          {years.length} {years.length === 1 ? "year" : "years"}
        </span>
      </div>

      {error && <p className="text-sm text-destructive px-6 mb-3 shrink-0">{error}</p>}

      <VirtualGrid
        items={years}
        minItemWidth={160}
        rowHeight={108}
        gap={12}
        px={24}
        paddingBottom={24}
        className="flex-1 min-h-0"
        loading={loading}
        empty={
          <p className="text-sm text-muted-foreground pt-2">
            No year tags found — scan a folder to populate the library.
          </p>
        }
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
    </div>
  );
}
