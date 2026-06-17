import { useEffect, useRef } from "react";
import { HashRouter, Routes, Route, Navigate, Outlet } from "react-router-dom";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { TooltipProvider } from "@/components/ui/tooltip";
import TitleBar from "@/components/title-bar";
import Sidebar from "@/components/sidebar";
import NowPlaying from "@/components/now-playing";
import CommandPalette from "@/components/command-palette";
import MiniPlayer from "@/components/mini-player";
import LyricsPanel from "@/components/lyrics-panel";
import { useCommandPalette } from "@/lib/command-palette-store";
import { useLyricsStore } from "@/lib/lyrics-store";
import { useThemeStore } from "@/lib/theme-store";
import { useScrollMemory } from "@/lib/scroll-memory";
import { openMiniPlayer } from "@/lib/ipc";
import AllSongs from "@/pages/all-songs";
import Albums from "@/pages/albums";
import Artists from "@/pages/artists";
import AlbumArtists from "@/pages/album-artists";
import Composers from "@/pages/composers";
import Genres from "@/pages/genres";
import Years from "@/pages/years";
import Library from "@/pages/library";
import FoldersView from "@/pages/folders-view";
import Playlists from "@/pages/playlists";
import PlaylistDetail from "@/pages/playlist-detail";
import SmartPlaylistDetail from "@/pages/smart-playlist-detail";
import RecentlyPlayed from "@/pages/recently-played";
import MostPlayed from "@/pages/most-played";
import Queue from "@/pages/queue";
import Settings from "@/pages/settings";

function isMiniWindow(): boolean {
  if (
    typeof window !== "undefined" &&
    (window as unknown as { __HALO_MINI__?: boolean }).__HALO_MINI__
  ) {
    return true;
  }
  try {
    return getCurrentWindow().label === "mini";
  } catch {
    return false;
  }
}

function Layout() {
  const toggle = useCommandPalette((s) => s.toggle);
  const lyricsOpen = useLyricsStore((s) => s.open);
  const initTheme = useThemeStore((s) => s.init);
  const mainScrollRef = useRef<HTMLDivElement>(null);
  useScrollMemory(mainScrollRef);

  useEffect(() => {
    initTheme();
  }, [initTheme]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        toggle();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "m") {
        e.preventDefault();
        openMiniPlayer().catch(console.error);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [toggle]);

  return (
    <div className="flex flex-col h-full bg-background text-foreground select-none">
      <TitleBar />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />
        <main className="flex-1 overflow-hidden relative select-text">
          <div ref={mainScrollRef} className={lyricsOpen ? "hidden" : "h-full overflow-auto"}>
            <Outlet />
          </div>
          {lyricsOpen && (
            <div className="absolute inset-0 overflow-hidden">
              <LyricsPanel />
            </div>
          )}
        </main>
      </div>
      <NowPlaying />
      <CommandPalette />
    </div>
  );
}

export default function App() {
  if (isMiniWindow()) {
    return (
      <TooltipProvider delay={400}>
        <MiniPlayer />
      </TooltipProvider>
    );
  }

  return (
    <TooltipProvider delay={400}>
      <HashRouter>
        <Routes>
          <Route element={<Layout />}>
            <Route index element={<Navigate to="/songs" replace />} />
            <Route path="/songs" element={<AllSongs />} />
            <Route path="/albums" element={<Albums />} />
            <Route path="/library" element={<Library />} />
            <Route path="/artists" element={<Artists />} />
            <Route path="/album-artists" element={<AlbumArtists />} />
            <Route path="/composers" element={<Composers />} />
            <Route path="/genres" element={<Genres />} />
            <Route path="/years" element={<Years />} />
            <Route path="/folders" element={<FoldersView />} />
            <Route path="/playlists" element={<Playlists />} />
            <Route path="/playlists/:id" element={<PlaylistDetail />} />
            {/* Smart playlists now live under the unified Playlists list; keep the
                detail route and redirect the old list path. */}
            <Route path="/smart-playlists" element={<Navigate to="/playlists" replace />} />
            <Route path="/smart-playlists/:id" element={<SmartPlaylistDetail />} />
            <Route path="/recently-played" element={<RecentlyPlayed />} />
            <Route path="/most-played" element={<MostPlayed />} />
            <Route path="/queue" element={<Queue />} />
            <Route path="/settings" element={<Settings />} />
          </Route>
        </Routes>
      </HashRouter>
    </TooltipProvider>
  );
}
