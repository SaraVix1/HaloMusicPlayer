import { useEffect, useRef, useState } from "react";
import { open as openFilePicker } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Disc, FolderOpen, ImageOff, Link2, Loader2, Search, X } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  extractTrackArt,
  fetchArtFromUrl,
  getTrackFullMetadata,
  processArt,
  saveTrackMetadata,
  searchCoverArt,
  type CoverSuggestion,
  type FullTrackMetadata,
} from "@/lib/ipc";

// ── Language data ─────────────────────────────────────────────────────────────

const LANGUAGES = [
  { code: "tam", label: "Tamil" },
  { code: "eng", label: "English" },
  { code: "tel", label: "Telugu" },
  { code: "mal", label: "Malayalam" },
  { code: "hin", label: "Hindi" },
  { code: "kan", label: "Kannada" },
  { code: "ben", label: "Bengali" },
  { code: "mar", label: "Marathi" },
  { code: "guj", label: "Gujarati" },
  { code: "pan", label: "Punjabi" },
  { code: "urd", label: "Urdu" },
  { code: "ara", label: "Arabic" },
  { code: "fra", label: "French" },
  { code: "deu", label: "German" },
  { code: "spa", label: "Spanish" },
  { code: "por", label: "Portuguese" },
  { code: "ita", label: "Italian" },
  { code: "rus", label: "Russian" },
  { code: "jpn", label: "Japanese" },
  { code: "kor", label: "Korean" },
  { code: "zho", label: "Chinese" },
  { code: "tha", label: "Thai" },
  { code: "vie", label: "Vietnamese" },
  { code: "ind", label: "Indonesian" },
  { code: "pol", label: "Polish" },
  { code: "nld", label: "Dutch" },
  { code: "swe", label: "Swedish" },
];

// ── Types ─────────────────────────────────────────────────────────────────────

interface Props {
  trackId: number | null;
  onClose: () => void;
  onSaved: (trackId: number) => void;
}

type ArtMode = "display" | "url" | "online" | "crop";

interface Draft {
  title: string;
  album_name: string;
  artists: string;
  album_artists: string;
  composers: string;
  genres: string;
  year: string;
  track_number: string;
  track_total: string;
  disc_number: string;
  disc_total: string;
  comment: string;
  publisher: string;
  copyright: string;
  language: string;
  new_art_path: string | null;
  art_preview: string | null;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

const join = (v: string[] | null | undefined) => (v ?? []).join(", ");
const split = (s: string) =>
  s
    .split(",")
    .map((x) => x.trim())
    .filter(Boolean);
const toNum = (s: string): number | null => {
  const n = parseInt(s, 10);
  return isNaN(n) || n < 1 ? null : n;
};

function toDraft(m: FullTrackMetadata): Draft {
  return {
    title: m.title ?? "",
    album_name: m.album_name ?? "",
    artists: join(m.artists),
    album_artists: join(m.album_artists),
    composers: join(m.composers),
    genres: join(m.genres),
    year: m.year != null ? String(m.year) : "",
    track_number: m.track_number != null ? String(m.track_number) : "",
    track_total: m.track_total != null ? String(m.track_total) : "",
    disc_number: m.disc_number != null ? String(m.disc_number) : "",
    disc_total: m.disc_total != null ? String(m.disc_total) : "",
    comment: m.comment ?? "",
    publisher: m.publisher ?? "",
    copyright: m.copyright ?? "",
    language: m.language ?? "",
    new_art_path: null,
    art_preview: null,
  };
}

function looksLikeImageUrl(text: string): boolean {
  try {
    const url = new URL(text.trim());
    const p = url.pathname.toLowerCase();
    return (
      /\.(jpe?g|png|gif|bmp|webp)(\?|$)/.test(p) ||
      /\/(i\.|img\.|image\.)/.test(url.href)
    );
  } catch {
    return false;
  }
}

// ── Main component ────────────────────────────────────────────────────────────

export default function MetadataEditorDialog({ trackId, onClose, onSaved }: Props) {
  const open = trackId !== null;

  const [meta, setMeta] = useState<FullTrackMetadata | null>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Art panel
  const [artMode, setArtMode] = useState<ArtMode>("display");
  const [urlInput, setUrlInput] = useState("");
  const [urlLoading, setUrlLoading] = useState(false);
  const [urlError, setUrlError] = useState<string | null>(null);
  const [extracting, setExtracting] = useState(false);
  const [suggestions, setSuggestions] = useState<CoverSuggestion[] | null>(null);
  const [suggestionsLoading, setSuggestionsLoading] = useState(false);
  const [suggestionsError, setSuggestionsError] = useState<string | null>(null);
  const [pickingIdx, setPickingIdx] = useState<number | null>(null);
  // Crop
  const [cropSrc, setCropSrc] = useState<string | null>(null);      // display URL
  const [cropRawPath, setCropRawPath] = useState<string | null>(null); // local FS path
  const [cropProcessing, setCropProcessing] = useState(false);

  const titleRef = useRef<HTMLInputElement>(null);

  // ── Load metadata ──────────────────────────────────────────────────────────

  useEffect(() => {
    if (!open || trackId === null) return;
    setLoading(true);
    setError(null);
    setMeta(null);
    setDraft(null);
    setArtMode("display");
    setUrlInput("");
    setSuggestions(null);
    setCropSrc(null);
    setCropRawPath(null);
    getTrackFullMetadata(trackId)
      .then((m) => {
        setMeta(m);
        setDraft(toDraft(m));
        setTimeout(() => titleRef.current?.focus(), 50);
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [open, trackId]);

  // ── Online search auto-trigger ─────────────────────────────────────────────

  useEffect(() => {
    if (artMode !== "online" || !draft) return;
    setSuggestions(null);
    setSuggestionsError(null);
    setSuggestionsLoading(true);
    // Prefer first album artist, fall back to first track artist
    const albumArtists = split(draft.album_artists);
    const trackArtists = split(draft.artists);
    const artist = albumArtists[0] ?? trackArtists[0] ?? "";
    const album = draft.album_name.trim();
    searchCoverArt(artist, album)
      .then(setSuggestions)
      .catch((e) => setSuggestionsError(String(e)))
      .finally(() => setSuggestionsLoading(false));
  }, [artMode]);

  // ── Clipboard URL auto-fill ────────────────────────────────────────────────

  useEffect(() => {
    if (artMode !== "url") return;
    navigator.clipboard
      .readText()
      .then((text) => {
        if (looksLikeImageUrl(text)) setUrlInput(text.trim());
      })
      .catch(() => {});
  }, [artMode]);

  // ── Field helper ──────────────────────────────────────────────────────────

  const set = (key: keyof Draft, value: string) =>
    setDraft((prev) => prev && { ...prev, [key]: value });

  // ── Art helpers ───────────────────────────────────────────────────────────

  const goToCrop = (localPath: string) => {
    setCropSrc(convertFileSrc(localPath));
    setCropRawPath(localPath);
    setArtMode("crop");
  };

  const setArt = (path: string) =>
    setDraft((prev) =>
      prev ? { ...prev, new_art_path: path, art_preview: convertFileSrc(path) } : prev,
    );

  const pickLocal = async () => {
    const result = await openFilePicker({
      multiple: false,
      filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "bmp", "gif", "webp"] }],
    });
    if (typeof result === "string" && result) goToCrop(result);
  };

  const extractFromFile = async () => {
    if (trackId === null) return;
    setExtracting(true);
    try {
      const path = await extractTrackArt(trackId);
      goToCrop(path);
    } catch (e) {
      setError(String(e));
    } finally {
      setExtracting(false);
    }
  };

  const loadFromUrl = async () => {
    const u = urlInput.trim();
    if (!u) return;
    setUrlLoading(true);
    setUrlError(null);
    try {
      const path = await fetchArtFromUrl(u);
      setUrlInput("");
      goToCrop(path);
    } catch (e) {
      setUrlError(String(e));
    } finally {
      setUrlLoading(false);
    }
  };

  const pickSuggestion = async (s: CoverSuggestion, idx: number) => {
    setPickingIdx(idx);
    try {
      const path = await fetchArtFromUrl(s.full_url).catch(() =>
        fetchArtFromUrl(s.thumbnail_url),
      );
      goToCrop(path);
      setArtMode("crop");
    } catch (e) {
      setSuggestionsError(String(e));
    } finally {
      setPickingIdx(null);
    }
  };

  const clearArt = () =>
    setDraft((prev) =>
      prev ? { ...prev, new_art_path: null, art_preview: null } : prev,
    );

  // ── Crop confirm ──────────────────────────────────────────────────────────

  const handleCropConfirm = async (
    naturalCrop: { x: number; y: number; w: number; h: number } | null,
  ) => {
    if (!cropRawPath) return;
    setCropProcessing(true);
    try {
      const path = await processArt(
        cropRawPath,
        naturalCrop?.x ?? 0,
        naturalCrop?.y ?? 0,
        naturalCrop?.w ?? 0,
        naturalCrop?.h ?? 0,
      );
      setArt(path);
      setArtMode("display");
    } catch (e) {
      setError(String(e));
      setArtMode("display");
    } finally {
      setCropProcessing(false);
      setCropSrc(null);
      setCropRawPath(null);
    }
  };

  // ── Save ──────────────────────────────────────────────────────────────────

  const handleSave = async () => {
    if (!draft || trackId === null) return;
    setSaving(true);
    setError(null);
    try {
      await saveTrackMetadata(trackId, {
        title: draft.title.trim() || null,
        album_name: draft.album_name.trim() || null,
        artists: split(draft.artists),
        album_artists: split(draft.album_artists),
        composers: split(draft.composers),
        genres: split(draft.genres),
        year: toNum(draft.year),
        track_number: toNum(draft.track_number),
        track_total: toNum(draft.track_total),
        disc_number: toNum(draft.disc_number),
        disc_total: toNum(draft.disc_total),
        comment: draft.comment.trim() || null,
        publisher: draft.publisher.trim() || null,
        copyright: draft.copyright.trim() || null,
        language: draft.language.trim() || null,
        new_art_path: draft.new_art_path,
      });
      onSaved(trackId);
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  // ── Derived ───────────────────────────────────────────────────────────────

  const currentArtSrc =
    draft?.art_preview ??
    (meta?.album_art_path ? convertFileSrc(meta.album_art_path) : null);

  const isCropMode = artMode === "crop";

  // ── Render ────────────────────────────────────────────────────────────────

  return (
    <Dialog open={open} onOpenChange={(o) => !o && !saving && onClose()}>
      <DialogContent
        className="sm:max-w-[640px] w-full"
        onKeyDown={(e) => {
          if (e.key === "Enter" && (e.ctrlKey || e.metaKey) && !isCropMode) handleSave();
        }}
      >
        <DialogHeader>
          <DialogTitle>Edit metadata</DialogTitle>
        </DialogHeader>

        {error && <p className="text-sm text-destructive -mt-1 mb-1">{error}</p>}

        {/* ── Crop mode ───────────────────────────────────────────────── */}
        {isCropMode && cropSrc && (
          <CropView
            src={cropSrc}
            processing={cropProcessing}
            onConfirm={handleCropConfirm}
          />
        )}

        {/* ── Normal mode ─────────────────────────────────────────────── */}
        {!isCropMode && (
          <>
            {loading || !draft ? (
              <p className="text-sm text-muted-foreground py-6 text-center">Loading…</p>
            ) : (
              <div className="flex gap-4 min-h-0">
                {/* Art column */}
                <div className="w-[152px] shrink-0 flex flex-col gap-2">
                  <ArtPreview src={currentArtSrc} loading={extracting} />

                  {artMode === "display" && (
                    <div className="flex gap-1 flex-wrap justify-center">
                      <ArtBtn title="Pick from file" onClick={pickLocal}>
                        <FolderOpen size={13} />
                      </ArtBtn>
                      <ArtBtn title="Extract embedded art" onClick={extractFromFile} loading={extracting}>
                        <ImageOff size={13} />
                      </ArtBtn>
                      <ArtBtn title="From URL" onClick={() => setArtMode("url")}>
                        <Link2 size={13} />
                      </ArtBtn>
                      <ArtBtn title="Online suggestions" onClick={() => setArtMode("online")}>
                        <Search size={13} />
                      </ArtBtn>
                      {draft.new_art_path && (
                        <ArtBtn title="Undo art change" onClick={clearArt}>
                          <X size={13} />
                        </ArtBtn>
                      )}
                    </div>
                  )}

                  {artMode === "url" && (
                    <>
                      <input
                        autoFocus
                        value={urlInput}
                        onChange={(e) => setUrlInput(e.target.value)}
                        onKeyDown={(e) => e.key === "Enter" && loadFromUrl()}
                        placeholder="Paste image URL…"
                        className="w-full bg-muted/40 border border-border rounded-md px-2 py-1 text-xs outline-none focus:ring-1 focus:ring-ring"
                      />
                      {urlError && <p className="text-xs text-destructive">{urlError}</p>}
                      <div className="flex gap-1.5">
                        <button
                          onClick={loadFromUrl}
                          disabled={!urlInput.trim() || urlLoading}
                          className="flex-1 rounded-md bg-primary text-primary-foreground text-xs py-1 disabled:opacity-50 flex items-center justify-center gap-1"
                        >
                          {urlLoading && <Loader2 size={11} className="animate-spin" />}
                          Load
                        </button>
                        <button
                          onClick={() => {
                            setArtMode("display");
                            setUrlInput("");
                            setUrlError(null);
                          }}
                          className="flex-1 rounded-md border border-border text-xs py-1 hover:bg-muted/40"
                        >
                          Cancel
                        </button>
                      </div>
                    </>
                  )}

                  {artMode === "online" && (
                    <>
                      <p className="text-xs text-muted-foreground font-medium leading-none">
                        Suggestions
                      </p>
                      {suggestionsLoading && (
                        <div className="flex items-center justify-center h-20">
                          <Loader2 size={18} className="animate-spin text-muted-foreground" />
                        </div>
                      )}
                      {suggestionsError && (
                        <p className="text-xs text-destructive">{suggestionsError}</p>
                      )}
                      {suggestions && !suggestionsLoading && (
                        suggestions.length === 0 ? (
                          <p className="text-xs text-muted-foreground">No results.</p>
                        ) : (
                          <div className="grid grid-cols-3 gap-1">
                            {suggestions.map((s, i) => (
                              <button
                                key={i}
                                title={`${s.title}${s.date ? ` (${s.date})` : ""}`}
                                onClick={() => pickSuggestion(s, i)}
                                disabled={pickingIdx !== null}
                                className="relative rounded overflow-hidden aspect-square bg-muted hover:ring-2 hover:ring-ring disabled:opacity-60"
                              >
                                {pickingIdx === i && (
                                  <div className="absolute inset-0 flex items-center justify-center bg-black/50">
                                    <Loader2 size={14} className="animate-spin text-white" />
                                  </div>
                                )}
                                <img
                                  src={s.thumbnail_url}
                                  className="w-full h-full object-cover"
                                  alt={s.title}
                                  onError={(e) => {
                                    (e.currentTarget.parentElement as HTMLElement).style.display = "none";
                                  }}
                                />
                              </button>
                            ))}
                          </div>
                        )
                      )}
                      <button
                        onClick={() => setArtMode("display")}
                        className="text-xs text-muted-foreground hover:text-foreground mt-auto text-left"
                      >
                        ← Back
                      </button>
                    </>
                  )}
                </div>

                {/* Fields */}
                <div className="flex-1 min-w-0 overflow-y-auto max-h-[62vh] pr-1 flex flex-col gap-2.5">
                  <F label="Title">
                    <input ref={titleRef} value={draft.title}
                      onChange={(e) => set("title", e.target.value)} placeholder="Track title" />
                  </F>
                  <F label="Artist">
                    <input value={draft.artists}
                      onChange={(e) => set("artists", e.target.value)} placeholder="Artist, comma-separated" />
                  </F>
                  <F label="Album artist">
                    <input value={draft.album_artists}
                      onChange={(e) => set("album_artists", e.target.value)} placeholder="Album artist, comma-separated" />
                  </F>
                  <F label="Album">
                    <input value={draft.album_name}
                      onChange={(e) => set("album_name", e.target.value)} placeholder="Album" />
                  </F>
                  <div className="grid grid-cols-2 gap-2.5">
                    <F label="Genre">
                      <input value={draft.genres}
                        onChange={(e) => set("genres", e.target.value)} placeholder="Genre, comma-separated" />
                    </F>
                    <F label="Composer">
                      <input value={draft.composers}
                        onChange={(e) => set("composers", e.target.value)} placeholder="Composer, comma-separated" />
                    </F>
                  </div>

                  {/* Year + Track # / total + Disc # / total */}
                  <div className="flex gap-2 items-end">
                    <F label="Year" className="w-[72px] shrink-0">
                      <input type="number" min={1} max={9999} value={draft.year}
                        onChange={(e) => set("year", e.target.value)} placeholder="Year" />
                    </F>
                    <F label="Track #" className="flex-1">
                      <div className="flex items-center gap-1">
                        <input className="!w-0 flex-1 min-w-0" type="number" min={1} value={draft.track_number}
                          onChange={(e) => set("track_number", e.target.value)} placeholder="#" />
                        <span className="text-muted-foreground text-xs shrink-0">/</span>
                        <input className="!w-0 flex-1 min-w-0" type="number" min={1} value={draft.track_total}
                          onChange={(e) => set("track_total", e.target.value)} placeholder="of" />
                      </div>
                    </F>
                    <F label="Disc #" className="flex-1">
                      <div className="flex items-center gap-1">
                        <input className="!w-0 flex-1 min-w-0" type="number" min={1} value={draft.disc_number}
                          onChange={(e) => set("disc_number", e.target.value)} placeholder="#" />
                        <span className="text-muted-foreground text-xs shrink-0">/</span>
                        <input className="!w-0 flex-1 min-w-0" type="number" min={1} value={draft.disc_total}
                          onChange={(e) => set("disc_total", e.target.value)} placeholder="of" />
                      </div>
                    </F>
                  </div>

                  <F label="Comment">
                    <textarea
                      value={draft.comment}
                      onChange={(e) => set("comment", e.target.value)}
                      placeholder="Comment"
                      rows={2}
                      className="w-full bg-muted/40 border border-border rounded-md px-2 py-1 text-sm outline-none focus:ring-1 focus:ring-ring resize-none"
                    />
                  </F>
                  <div className="grid grid-cols-2 gap-2.5">
                    <F label="Publisher / Label">
                      <input value={draft.publisher}
                        onChange={(e) => set("publisher", e.target.value)} placeholder="Record label" />
                    </F>
                    <F label="Copyright">
                      <input value={draft.copyright}
                        onChange={(e) => set("copyright", e.target.value)} placeholder="© 2025 …" />
                    </F>
                  </div>

                  <F label="Language">
                    <>
                      <input
                        list="lang-codes"
                        value={draft.language}
                        onChange={(e) => set("language", e.target.value)}
                        placeholder="e.g. eng"
                        maxLength={20}
                        className="w-full bg-muted/40 border border-border rounded-md px-2 py-1 text-sm outline-none focus:ring-1 focus:ring-ring"
                      />
                      <datalist id="lang-codes">
                        {LANGUAGES.map((l) => (
                          <option key={l.code} value={l.code}>
                            {l.label}
                          </option>
                        ))}
                      </datalist>
                    </>
                  </F>
                </div>
              </div>
            )}

            <DialogFooter className="-mx-4 -mb-4">
              <button
                onClick={onClose}
                disabled={saving}
                className="rounded-lg border border-border px-4 py-1.5 text-sm hover:bg-muted/40 disabled:opacity-50"
              >
                Cancel
              </button>
              <button
                onClick={handleSave}
                disabled={saving || loading || !draft}
                className="rounded-lg bg-primary text-primary-foreground px-4 py-1.5 text-sm disabled:opacity-50 flex items-center gap-1.5"
              >
                {saving && <Loader2 size={13} className="animate-spin" />}
                {saving ? "Saving…" : "Save"}
              </button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}

// ── Sub-components ────────────────────────────────────────────────────────────

function ArtPreview({ src, loading }: { src: string | null; loading?: boolean }) {
  return (
    <div className="relative w-full aspect-square rounded-md overflow-hidden bg-muted">
      {src ? (
        <img src={src} className="w-full h-full object-cover" alt="" />
      ) : (
        <div className="flex items-center justify-center w-full h-full text-muted-foreground/40">
          <Disc size={40} />
        </div>
      )}
      {loading && (
        <div className="absolute inset-0 flex items-center justify-center bg-black/40">
          <Loader2 size={20} className="animate-spin text-white" />
        </div>
      )}
    </div>
  );
}

function ArtBtn({
  title, onClick, loading, children,
}: {
  title: string; onClick: () => void; loading?: boolean; children: React.ReactNode;
}) {
  return (
    <button
      title={title}
      onClick={onClick}
      disabled={loading}
      className="w-7 h-7 flex items-center justify-center rounded-md border border-border text-muted-foreground hover:text-foreground hover:bg-muted/40 disabled:opacity-50"
    >
      {children}
    </button>
  );
}

function F({
  label, children, className,
}: {
  label: string; children: React.ReactNode; className?: string;
}) {
  return (
    <label className={`flex flex-col gap-1 ${className ?? ""}`}>
      <span className="text-xs text-muted-foreground font-medium leading-none">{label}</span>
      <div className="[&_input]:w-full [&_input]:bg-muted/40 [&_input]:border [&_input]:border-border [&_input]:rounded-md [&_input]:px-2 [&_input]:py-1 [&_input]:text-sm [&_input]:outline-none [&_input]:focus:ring-1 [&_input]:focus:ring-ring [&_input::-webkit-inner-spin-button]:appearance-none [&_input::-webkit-outer-spin-button]:appearance-none">
        {children}
      </div>
    </label>
  );
}

// ── Crop view ─────────────────────────────────────────────────────────────────

interface CropRect { x: number; y: number; w: number; h: number }

function CropView({
  src,
  processing,
  onConfirm,
}: {
  src: string;
  processing: boolean;
  onConfirm: (naturalCrop: CropRect | null) => void;
}) {
  const imgRef = useRef<HTMLImageElement>(null);
  const [naturalSize, setNaturalSize] = useState<{ w: number; h: number } | null>(null);
  const [dispSize, setDispSize] = useState<{ w: number; h: number } | null>(null);
  const [crop, setCrop] = useState<CropRect>({ x: 0, y: 0, w: 0, h: 0 });
  const [imgLoaded, setImgLoaded] = useState(false);
  const [imgError, setImgError] = useState(false);

  const dragRef = useRef<{
    handle: string;
    startX: number;
    startY: number;
    startCrop: CropRect;
  } | null>(null);

  const MAX_W = 540;
  const MAX_H = 360;
  const MIN_CROP = 30;

  const onImgLoad = () => {
    const img = imgRef.current!;
    const nw = img.naturalWidth;
    const nh = img.naturalHeight;
    setNaturalSize({ w: nw, h: nh });
    const scale = Math.min(MAX_W / nw, MAX_H / nh, 1);
    const dw = Math.round(nw * scale);
    const dh = Math.round(nh * scale);
    setDispSize({ w: dw, h: dh });
    const side = Math.min(dw, dh);
    setCrop({ x: Math.round((dw - side) / 2), y: Math.round((dh - side) / 2), w: side, h: side });
    setImgLoaded(true);
  };

  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      if (!dragRef.current || !dispSize) return;
      const { handle, startX, startY, startCrop } = dragRef.current;
      const dx = e.clientX - startX;
      const dy = e.clientY - startY;

      setCrop(() => {
        let { x, y, w, h } = startCrop;
        if (handle === "move") {
          x = clamp(x + dx, 0, dispSize.w - w);
          y = clamp(y + dy, 0, dispSize.h - h);
        } else {
          if (handle.includes("e")) w = clamp(w + dx, MIN_CROP, dispSize.w - x);
          if (handle.includes("s")) h = clamp(h + dy, MIN_CROP, dispSize.h - y);
          if (handle.includes("w")) {
            const nx = clamp(x + dx, 0, x + w - MIN_CROP);
            w += x - nx; x = nx;
          }
          if (handle.includes("n")) {
            const ny = clamp(y + dy, 0, y + h - MIN_CROP);
            h += y - ny; y = ny;
          }
        }
        return { x, y, w, h };
      });
    };
    const onUp = () => { dragRef.current = null; };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
    return () => {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
    };
  }, [dispSize]);

  const startDrag = (handle: string, e: React.MouseEvent) => {
    e.preventDefault();
    dragRef.current = { handle, startX: e.clientX, startY: e.clientY, startCrop: { ...crop } };
  };

  const confirm = () => {
    if (!naturalSize || !dispSize) return;
    const sx = naturalSize.w / dispSize.w;
    const sy = naturalSize.h / dispSize.h;
    onConfirm({
      x: Math.round(crop.x * sx),
      y: Math.round(crop.y * sy),
      w: Math.round(crop.w * sx),
      h: Math.round(crop.h * sy),
    });
  };

  const HS = 10; // handle size px
  const handles = [
    { key: "nw", style: { top: -HS / 2, left: -HS / 2, cursor: "nw-resize" } },
    { key: "n",  style: { top: -HS / 2, left: "50%", translate: "-50%", cursor: "n-resize" } },
    { key: "ne", style: { top: -HS / 2, right: -HS / 2, cursor: "ne-resize" } },
    { key: "e",  style: { top: "50%", right: -HS / 2, translate: "0 -50%", cursor: "e-resize" } },
    { key: "se", style: { bottom: -HS / 2, right: -HS / 2, cursor: "se-resize" } },
    { key: "s",  style: { bottom: -HS / 2, left: "50%", translate: "-50%", cursor: "s-resize" } },
    { key: "sw", style: { bottom: -HS / 2, left: -HS / 2, cursor: "sw-resize" } },
    { key: "w",  style: { top: "50%", left: -HS / 2, translate: "0 -50%", cursor: "w-resize" } },
  ] as const;

  return (
    <div className="flex flex-col items-center gap-3 py-1">
      <p className="text-xs text-muted-foreground self-start">
        Drag the crop box · resize from edges or corners
      </p>

      <div
        className="relative bg-black select-none overflow-hidden rounded"
        style={{ width: dispSize?.w ?? MAX_W, height: dispSize?.h ?? 200 }}
      >
        {!imgLoaded && !imgError && (
          <div className="absolute inset-0 flex items-center justify-center">
            <Loader2 size={24} className="animate-spin text-muted-foreground" />
          </div>
        )}
        {imgError && (
          <div className="absolute inset-0 flex items-center justify-center text-destructive text-sm">
            Failed to load image
          </div>
        )}
        <img
          ref={imgRef}
          src={src}
          draggable={false}
          onLoad={onImgLoad}
          onError={() => setImgError(true)}
          style={{ width: dispSize?.w, height: dispSize?.h, display: imgLoaded ? "block" : "none" }}
          alt=""
        />

        {imgLoaded && dispSize && (
          <>
            {/* Dark overlay (4 rects) */}
            <div className="pointer-events-none absolute inset-0">
              <div className="absolute bg-black/55" style={{ top: 0, left: 0, right: 0, height: crop.y }} />
              <div className="absolute bg-black/55" style={{ top: crop.y + crop.h, left: 0, right: 0, bottom: 0 }} />
              <div className="absolute bg-black/55" style={{ top: crop.y, left: 0, width: crop.x, height: crop.h }} />
              <div className="absolute bg-black/55" style={{ top: crop.y, left: crop.x + crop.w, right: 0, height: crop.h }} />
            </div>

            {/* Crop box */}
            <div
              className="absolute border border-white/80"
              style={{ left: crop.x, top: crop.y, width: crop.w, height: crop.h, cursor: "move" }}
              onMouseDown={(e) => startDrag("move", e)}
            >
              {handles.map(({ key, style }) => (
                <div
                  key={key}
                  className="absolute bg-white"
                  style={{ ...style, width: HS, height: HS }}
                  onMouseDown={(e) => { e.stopPropagation(); startDrag(key, e); }}
                />
              ))}
            </div>
          </>
        )}

        {processing && (
          <div className="absolute inset-0 flex items-center justify-center bg-black/60">
            <Loader2 size={28} className="animate-spin text-white" />
          </div>
        )}
      </div>

      <div className="flex gap-2">
        <button
          onClick={confirm}
          disabled={!imgLoaded || processing}
          className="rounded-lg bg-primary text-primary-foreground px-4 py-1.5 text-sm disabled:opacity-50 flex items-center gap-1.5"
        >
          {processing && <Loader2 size={13} className="animate-spin" />}
          Apply crop
        </button>
        <button
          onClick={() => onConfirm(null)}
          disabled={processing}
          className="rounded-lg border border-border px-4 py-1.5 text-sm hover:bg-muted/40 disabled:opacity-50"
        >
          Use full image
        </button>
      </div>
    </div>
  );
}

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}
