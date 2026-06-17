import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Play } from "lucide-react";
import { getAlbums, getTracks, setQueueAndPlay, type Album } from "@/lib/ipc";
import AlbumArt from "@/components/album-art";
import { VirtualGrid } from "@/components/virtual-grid";
import { useLibraryRefresh } from "@/lib/library-events";

export default function Albums() {
  const [albums, setAlbums] = useState<Album[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const refreshTick = useLibraryRefresh();

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getAlbums()
      .then((rows) => { if (!cancelled) setAlbums(rows); })
      .catch((e) => { if (!cancelled) setError(String(e)); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [refreshTick]);

  const playAlbum = async (name: string) => {
    try {
      const tracks = await getTracks({ album: name, sort: "track_number", direction: "asc" });
      if (tracks.length > 0) await setQueueAndPlay(tracks.map((t) => t.id), 0);
    } catch (e) {
      console.error(e);
    }
  };

  return (
    <div className="h-full flex flex-col">
      <div className="px-6 pt-6 pb-4 shrink-0 flex items-baseline justify-between gap-4">
        <h1 className="text-2xl font-semibold">Albums</h1>
        <span className="text-sm text-muted-foreground tabular-nums">
          {albums.length} {albums.length === 1 ? "album" : "albums"}
        </span>
      </div>

      {error && <p className="text-sm text-destructive px-6 mb-3 shrink-0">{error}</p>}

      <VirtualGrid
        items={albums}
        minItemWidth={160}
        rowHeight={240}
        gap={20}
        px={24}
        paddingBottom={24}
        className="flex-1 min-h-0"
        loading={loading}
        empty={
          <p className="text-sm text-muted-foreground pt-2">
            No albums yet — scan a folder to populate the library.
          </p>
        }
        renderItem={(album) => (
          <div key={album.name} className="group flex flex-col gap-2">
            <div className="relative">
              <Link
                to={`/songs?album=${encodeURIComponent(album.name)}&sort=track_number&dir=asc`}
                aria-label={`Open ${album.name}`}
              >
                <AlbumArt
                  path={album.album_art_path}
                  size={160}
                  rounded="md"
                  className="w-full aspect-square h-auto group-hover:opacity-80 transition-opacity"
                />
              </Link>
              <button
                onClick={(e) => { e.preventDefault(); playAlbum(album.name); }}
                className="absolute bottom-2 right-2 w-10 h-10 rounded-full bg-primary text-primary-foreground flex items-center justify-center opacity-0 group-hover:opacity-100 shadow-lg hover:scale-105 transition-all"
                aria-label={`Play ${album.name}`}
              >
                <Play size={16} className="ml-0.5" />
              </button>
            </div>
            <div className="min-w-0">
              <Link
                to={`/songs?album=${encodeURIComponent(album.name)}&sort=track_number&dir=asc`}
                className="text-sm font-medium truncate block hover:underline"
              >
                {album.name}
              </Link>
              <p className="text-xs text-muted-foreground truncate">
                {album.album_artists.length > 0
                  ? album.album_artists.join(", ")
                  : `${album.track_count} ${album.track_count === 1 ? "track" : "tracks"}`}
              </p>
            </div>
          </div>
        )}
      />
    </div>
  );
}
