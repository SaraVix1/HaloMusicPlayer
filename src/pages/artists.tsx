import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { getArtists, type Artist } from "@/lib/ipc";
import AlbumArt from "@/components/album-art";
import { VirtualGrid } from "@/components/virtual-grid";
import { initials } from "@/lib/format";
import { useLibraryRefresh } from "@/lib/library-events";

function ArtistAvatar({ artist }: { artist: Artist }) {
  if (artist.album_art_path) {
    return (
      <AlbumArt
        path={artist.album_art_path}
        size={140}
        rounded="full"
        className="w-full aspect-square h-auto"
      />
    );
  }
  return (
    <div className="w-full aspect-square rounded-full bg-muted text-muted-foreground flex items-center justify-center text-3xl font-medium">
      {initials(artist.name) || "?"}
    </div>
  );
}

export default function Artists() {
  const [artists, setArtists] = useState<Artist[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const refreshTick = useLibraryRefresh();

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getArtists()
      .then((rows) => { if (!cancelled) setArtists(rows); })
      .catch((e) => { if (!cancelled) setError(String(e)); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [refreshTick]);

  return (
    <div className="h-full flex flex-col">
      <div className="px-6 pt-6 pb-4 shrink-0 flex items-baseline justify-between gap-4">
        <h1 className="text-2xl font-semibold">Artists</h1>
        <span className="text-sm text-muted-foreground tabular-nums">
          {artists.length} {artists.length === 1 ? "artist" : "artists"}
        </span>
      </div>

      {error && <p className="text-sm text-destructive px-6 mb-3 shrink-0">{error}</p>}

      <VirtualGrid
        items={artists}
        minItemWidth={140}
        rowHeight={210}
        gap={20}
        px={24}
        paddingBottom={24}
        className="flex-1 min-h-0"
        loading={loading}
        empty={
          <p className="text-sm text-muted-foreground pt-2">
            No artists yet — scan a folder to populate the library.
          </p>
        }
        renderItem={(artist) => (
          <Link
            key={artist.id}
            to={`/songs?artist=${encodeURIComponent(artist.name)}`}
            className="group flex flex-col items-center gap-2 text-center"
          >
            <div className="w-full group-hover:opacity-90 transition-opacity">
              <ArtistAvatar artist={artist} />
            </div>
            <div className="min-w-0 w-full">
              <p className="text-sm font-medium truncate">{artist.name}</p>
              <p className="text-xs text-muted-foreground tabular-nums">
                {artist.track_count} {artist.track_count === 1 ? "track" : "tracks"}
              </p>
            </div>
          </Link>
        )}
      />
    </div>
  );
}
