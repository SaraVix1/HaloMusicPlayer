import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { X, Pencil, Check, Loader2, CloudDownload, Search } from "lucide-react";
import {
  getLyrics,
  saveLyrics,
  fetchLyricsOnline,
  searchLyricsProviders,
  seekTo,
  type LyricsResult,
  type LyricsCandidate,
} from "@/lib/ipc";
import { useLyricsStore } from "@/lib/lyrics-store";
import { usePlayerStore } from "@/lib/player-store";
import { cn } from "@/lib/utils";

const SOURCE_LABEL: Record<string, string> = {
  database: "From tag / saved",
  lrc_file: "From .lrc file",
  lrclib: "From LRCLIB",
  jiosaavn: "From JioSaavn",
  lyricsovh: "From Lyrics.ovh",
};

const PROVIDER_LABEL: Record<string, string> = {
  lrclib: "LRCLIB",
  jiosaavn: "JioSaavn",
  lyricsovh: "Lyrics.ovh",
};

const EMPTY: LyricsResult = { source: "none", synced: false, lines: [] };

interface SearchForm {
  title: string;
  artist: string;
  album: string;
}

export default function LyricsPanel() {
  const hide = useLyricsStore((s) => s.hide);
  const track = usePlayerStore((s) => s.current_track);
  const positionMs = usePlayerStore((s) => s.position_ms);

  const [lyrics, setLyrics] = useState<LyricsResult>(EMPTY);
  const [loading, setLoading] = useState(false);
  const [fetchingOnline, setFetchingOnline] = useState(false);
  const [onlineNotFound, setOnlineNotFound] = useState(false);
  const [onlineError, setOnlineError] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [saving, setSaving] = useState(false);
  const [searching, setSearching] = useState(false);
  const [searchForm, setSearchForm] = useState<SearchForm>({ title: "", artist: "", album: "" });
  const [candidates, setCandidates] = useState<LyricsCandidate[]>([]);
  const [searchDone, setSearchDone] = useState(false);

  const activeLineRef = useRef<HTMLParagraphElement>(null);

  const resetSearchState = () => {
    setCandidates([]);
    setSearchDone(false);
    setOnlineNotFound(false);
    setOnlineError(null);
  };

  // Auto-fetch via JioSaavn (called when local result is empty).
  const runOnlineFetch = useCallback(async (fetcher: () => Promise<LyricsResult>) => {
    setFetchingOnline(true);
    setOnlineNotFound(false);
    setOnlineError(null);
    try {
      const result = await fetcher();
      setLyrics(result);
      if (result.source === "none" || result.source === "not_found") {
        setOnlineNotFound(true);
      }
    } catch (e) {
      setOnlineError(String(e));
    } finally {
      setFetchingOnline(false);
    }
  }, []);

  const fetchLocal = useCallback(async (trackId: number) => {
    setLoading(true);
    setFetchingOnline(false);
    setEditing(false);
    setSearching(false);
    resetSearchState();

    let localResult: LyricsResult = EMPTY;
    try {
      localResult = await getLyrics(trackId);
    } catch {
      localResult = EMPTY;
    }

    setLyrics(localResult);
    setLoading(false);

    // Auto-fetch from JioSaavn only when DB lyrics are NULL (never searched).
    // "" sentinel means "already searched, nothing found" — skip.
    if (localResult.source === "none") {
      setFetchingOnline(true);
      try {
        const online = await fetchLyricsOnline(trackId);
        setLyrics(online);
        if (online.source === "none" || online.source === "not_found") {
          setOnlineNotFound(true);
        }
      } catch (e) {
        setOnlineError(String(e));
      } finally {
        setFetchingOnline(false);
      }
    }
  }, []);

  useEffect(() => {
    if (track) {
      fetchLocal(track.track_id);
    } else {
      setLyrics(EMPTY);
      setEditing(false);
      setSearching(false);
      resetSearchState();
    }
  }, [track?.track_id, fetchLocal]);

  const enterSearchMode = () => {
    setSearchForm({
      title: track?.title ?? "",
      artist: track?.artists.join(", ") ?? "",
      album: track?.album_name ?? "",
    });
    setSearching(true);
    resetSearchState();
  };

  // Manual search: query all providers in parallel, show candidates for user to pick.
  const runProviderSearch = useCallback(async () => {
    if (!track || !searchForm.title.trim()) return;
    setFetchingOnline(true);
    setCandidates([]);
    setSearchDone(false);
    setOnlineNotFound(false);
    setOnlineError(null);
    try {
      const results = await searchLyricsProviders(
        track.track_id,
        searchForm.title,
        searchForm.artist,
        searchForm.album,
      );
      setCandidates(results);
      setSearchDone(true);
      if (results.length === 0) setOnlineNotFound(true);
    } catch (e) {
      setOnlineError(String(e));
    } finally {
      setFetchingOnline(false);
    }
  }, [track, searchForm]);

  const applyCandidate = useCallback(
    async (candidate: LyricsCandidate) => {
      if (!track) return;
      try {
        await saveLyrics(track.track_id, candidate.lyrics);
        await fetchLocal(track.track_id);
        setSearching(false);
        resetSearchState();
      } catch (e) {
        setOnlineError(String(e));
      }
    },
    [track, fetchLocal],
  );

  // Active line: last line whose time_ms <= positionMs
  const activeIndex = useMemo(() => {
    if (!lyrics.synced || lyrics.lines.length === 0) return -1;
    let idx = -1;
    for (let i = 0; i < lyrics.lines.length; i++) {
      if (lyrics.lines[i].time_ms <= positionMs) idx = i;
      else break;
    }
    return idx;
  }, [lyrics, positionMs]);

  useEffect(() => {
    if (activeIndex >= 0 && activeLineRef.current) {
      activeLineRef.current.scrollIntoView({ behavior: "smooth", block: "center" });
    }
  }, [activeIndex]);

  const startEdit = () => {
    const raw = lyrics.lines
      .map((l) => (lyrics.synced ? `[${formatLrcTime(l.time_ms)}]${l.text}` : l.text))
      .join("\n");
    setDraft(raw);
    setEditing(true);
  };

  const commitEdit = async () => {
    if (!track) return;
    setSaving(true);
    try {
      await saveLyrics(track.track_id, draft);
      await fetchLocal(track.track_id);
      setEditing(false);
    } catch {
      // keep editing open on error
    } finally {
      setSaving(false);
    }
  };

  const hasLyrics =
    lyrics.source !== "none" && lyrics.source !== "not_found" && lyrics.lines.length > 0;

  return (
    <div className="flex flex-col h-full bg-background">
      {/* Header */}
      <div className="flex items-center justify-between px-6 py-3 border-b border-border shrink-0">
        <div className="flex items-center gap-3 min-w-0">
          <span className="text-sm font-medium truncate">
            {track ? (track.title ?? "Unknown track") : "No track playing"}
          </span>
          {hasLyrics && !searching && (
            <span className="text-xs text-muted-foreground shrink-0">
              {SOURCE_LABEL[lyrics.source] ?? lyrics.source}
            </span>
          )}
        </div>
        <div className="flex items-center gap-1 shrink-0 ml-4">
          {track && !editing && !searching && (
            <>
              <button
                onClick={() => runOnlineFetch(() => fetchLyricsOnline(track.track_id))}
                disabled={fetchingOnline}
                className="p-1.5 text-muted-foreground hover:text-foreground disabled:opacity-40 transition-colors rounded"
                aria-label="Fetch lyrics online"
                title="Fetch lyrics online"
              >
                {fetchingOnline ? (
                  <Loader2 size={14} className="animate-spin" />
                ) : (
                  <CloudDownload size={14} />
                )}
              </button>
              <button
                onClick={enterSearchMode}
                className="p-1.5 text-muted-foreground hover:text-foreground transition-colors rounded"
                aria-label="Search all providers"
                title="Search all providers"
              >
                <Search size={14} />
              </button>
              <button
                onClick={startEdit}
                className="p-1.5 text-muted-foreground hover:text-foreground transition-colors rounded"
                aria-label="Edit lyrics"
              >
                <Pencil size={14} />
              </button>
            </>
          )}
          {(editing || searching) && (
            <button
              onClick={() => {
                setEditing(false);
                setSearching(false);
                resetSearchState();
              }}
              className="p-1.5 text-muted-foreground hover:text-foreground transition-colors rounded"
              aria-label="Cancel"
            >
              <X size={14} />
            </button>
          )}
          {editing && (
            <button
              onClick={commitEdit}
              disabled={saving}
              className="p-1.5 text-primary hover:text-primary/80 disabled:opacity-50 transition-colors rounded"
              aria-label="Save lyrics"
            >
              {saving ? <Loader2 size={14} className="animate-spin" /> : <Check size={14} />}
            </button>
          )}
          {!editing && !searching && (
            <button
              onClick={hide}
              className="p-1.5 text-muted-foreground hover:text-foreground transition-colors rounded"
              aria-label="Close lyrics"
            >
              <X size={14} />
            </button>
          )}
        </div>
      </div>

      {/* Body */}
      <div className="flex-1 overflow-y-auto">
        {loading ? (
          <div className="flex items-center justify-center h-full">
            <Loader2 size={20} className="animate-spin text-muted-foreground" />
          </div>
        ) : editing ? (
          <div className="p-6 h-full flex flex-col gap-3">
            <p className="text-xs text-muted-foreground">
              Plain text or LRC format (e.g.{" "}
              <code className="font-mono">[00:12.50]Line text</code>)
            </p>
            <textarea
              className="flex-1 w-full bg-muted/40 border border-border rounded-md p-3 text-sm font-mono resize-none focus:outline-none focus:ring-1 focus:ring-ring"
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              placeholder="Paste lyrics here..."
              spellCheck={false}
            />
          </div>
        ) : searching ? (
          <div className="p-5 flex flex-col gap-4">
            {/* Search form */}
            <div className="flex flex-col gap-3">
              {(["title", "artist", "album"] as const).map((key) => (
                <div key={key} className="flex flex-col gap-1">
                  <label className="text-xs text-muted-foreground capitalize">
                    {key}
                    {key === "album" ? " (optional)" : ""}
                  </label>
                  <input
                    type="text"
                    value={searchForm[key]}
                    onChange={(e) => setSearchForm((f) => ({ ...f, [key]: e.target.value }))}
                    onKeyDown={(e) => e.key === "Enter" && runProviderSearch()}
                    className="bg-muted/40 border border-border rounded-md px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-ring"
                  />
                </div>
              ))}
              <button
                onClick={runProviderSearch}
                disabled={fetchingOnline || !searchForm.title.trim()}
                className="flex items-center justify-center gap-2 px-4 py-2 rounded-full text-sm bg-primary text-primary-foreground hover:opacity-90 disabled:opacity-50 transition-opacity self-start mt-1"
              >
                {fetchingOnline ? (
                  <Loader2 size={14} className="animate-spin" />
                ) : (
                  <Search size={14} />
                )}
                {fetchingOnline ? "Searching all providers…" : "Search all providers"}
              </button>
            </div>

            {/* Results */}
            {onlineError && <p className="text-xs text-destructive">{onlineError}</p>}
            {searchDone && candidates.length === 0 && !onlineError && (
              <p className="text-sm text-muted-foreground">
                No results found — try different values.
              </p>
            )}
            {candidates.length > 0 && (
              <div className="flex flex-col gap-3">
                <p className="text-xs text-muted-foreground">
                  {candidates.length} result{candidates.length !== 1 ? "s" : ""} — pick one to
                  use:
                </p>
                {candidates.map((c, i) => (
                  <div
                    key={i}
                    className="border border-border rounded-lg p-3 flex flex-col gap-2"
                  >
                    <div className="flex items-center gap-2 flex-wrap">
                      <span className="text-xs font-medium bg-muted px-2 py-0.5 rounded-full shrink-0">
                        {PROVIDER_LABEL[c.provider] ?? c.provider}
                      </span>
                      {c.synced && (
                        <span className="text-xs text-primary font-medium shrink-0">Synced</span>
                      )}
                      {c.label && (
                        <span className="text-xs text-foreground/80 truncate">{c.label}</span>
                      )}
                    </div>
                    <p className="text-xs text-muted-foreground font-mono whitespace-pre-line leading-relaxed">
                      {c.snippet ? c.snippet + "…" : "(no preview)"}
                    </p>
                    <button
                      onClick={() => applyCandidate(c)}
                      className="self-end text-xs px-3 py-1.5 rounded-full bg-primary text-primary-foreground hover:opacity-90 transition-opacity"
                    >
                      Use this
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>
        ) : !hasLyrics ? (
          <div className="flex flex-col items-center justify-center h-full gap-3 text-muted-foreground">
            {fetchingOnline ? (
              <>
                <Loader2 size={20} className="animate-spin" />
                <p className="text-sm">Searching for lyrics…</p>
              </>
            ) : (
              <>
                <p className="text-sm">
                  {onlineNotFound || lyrics.source === "not_found"
                    ? "No lyrics found online."
                    : "No lyrics found."}
                </p>
                {onlineError && (
                  <p className="text-xs text-destructive max-w-xs text-center">{onlineError}</p>
                )}
                {track && (
                  <div className="flex flex-col items-center gap-2 mt-1">
                    <button
                      onClick={enterSearchMode}
                      className="flex items-center gap-2 px-4 py-2 rounded-full text-sm border border-border hover:bg-muted transition-colors"
                    >
                      <Search size={14} />
                      Search all providers
                    </button>
                    <button
                      onClick={startEdit}
                      className="text-xs underline hover:text-foreground transition-colors"
                    >
                      Add lyrics manually
                    </button>
                  </div>
                )}
              </>
            )}
          </div>
        ) : (
          <div className="px-8 py-12 flex flex-col items-center gap-1 text-center">
            {lyrics.lines.map((line, i) => {
              const isActive = lyrics.synced && i === activeIndex;
              const isPast = lyrics.synced && i < activeIndex;
              return (
                <p
                  key={i}
                  ref={isActive ? activeLineRef : undefined}
                  onClick={
                    lyrics.synced ? () => seekTo(line.time_ms).catch(console.error) : undefined
                  }
                  className={cn(
                    "text-lg leading-relaxed transition-all duration-300 max-w-2xl",
                    isActive
                      ? "text-foreground font-semibold scale-105"
                      : isPast
                        ? "text-muted-foreground/50"
                        : "text-muted-foreground",
                    lyrics.synced && "cursor-pointer hover:text-foreground",
                  )}
                >
                  {line.text}
                </p>
              );
            })}
            <div className="h-64 shrink-0" />
          </div>
        )}
      </div>
    </div>
  );
}

function formatLrcTime(ms: number): string {
  const totalSecs = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSecs / 60);
  const seconds = totalSecs % 60;
  const centis = Math.floor((ms % 1000) / 10);
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}.${String(centis).padStart(2, "0")}`;
}
