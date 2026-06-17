import { useCallback, useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { Plus, Trash2, ChevronLeft, RefreshCw, Play, Shuffle } from "lucide-react";
import {
  getSmartPlaylist,
  getSmartPlaylistTracks,
  updateSmartPlaylist,
  deleteSmartPlaylist,
  setSmartPlaylistRules,
  setQueueAndPlay,
  type SmartPlaylist,
  type SmartPlaylistRule,
  type SmartTrack,
} from "@/lib/ipc";
import { formatDuration } from "@/lib/format";
import { cn } from "@/lib/utils";
import { useLibraryRefresh } from "@/lib/library-events";
import AlbumArt from "@/components/album-art";

// ---------------------------------------------------------------------------
// Rule editor metadata
// ---------------------------------------------------------------------------

const BUILTIN_IDS = new Set([1, 2, 3, 4, 5]);

type FieldType = "text" | "number" | "date";

const FIELDS: { value: string; label: string; type: FieldType }[] = [
  { value: "title",          label: "Title",           type: "text"   },
  { value: "artist",         label: "Artist",          type: "text"   },
  { value: "album",          label: "Album",           type: "text"   },
  { value: "album_artist",   label: "Album artist",    type: "text"   },
  { value: "genre",          label: "Genre",           type: "text"   },
  { value: "composer",       label: "Composer",        type: "text"   },
  { value: "year",           label: "Year",            type: "number" },
  { value: "rating",         label: "Rating (0–5)",    type: "number" },
  { value: "play_count",     label: "Play count",      type: "number" },
  { value: "skip_count",     label: "Skip count",      type: "number" },
  { value: "duration_ms",    label: "Duration (ms)",   type: "number" },
  { value: "date_added",     label: "Date added",      type: "date"   },
  { value: "last_played_at", label: "Last played",     type: "date"   },
];

const OPERATORS_TEXT    = [
  { value: "contains",     label: "contains"     },
  { value: "not_contains", label: "doesn't contain" },
  { value: "is",           label: "is"           },
  { value: "is_not",       label: "is not"       },
];
const OPERATORS_NUMBER  = [
  { value: "eq",    label: "="  },
  { value: "not_eq",label: "≠"  },
  { value: "gt",    label: ">"  },
  { value: "gte",   label: ">=" },
  { value: "lt",    label: "<"  },
  { value: "lte",   label: "<=" },
];
const OPERATORS_DATE    = [
  { value: "in_last_days", label: "in the last (days)" },
  { value: "gt",           label: "after"              },
  { value: "lt",           label: "before"             },
];

function operatorsForField(field: string) {
  const f = FIELDS.find((x) => x.value === field);
  if (!f) return OPERATORS_TEXT;
  if (f.type === "number") return OPERATORS_NUMBER;
  if (f.type === "date")   return OPERATORS_DATE;
  return OPERATORS_TEXT;
}

function defaultOperator(field: string): string {
  return operatorsForField(field)[0].value;
}

const SORT_FIELDS = [
  { value: "title",          label: "Title"       },
  { value: "artist",         label: "Artist"      },
  { value: "album",          label: "Album"       },
  { value: "year",           label: "Year"        },
  { value: "rating",         label: "Rating"      },
  { value: "play_count",     label: "Play count"  },
  { value: "duration_ms",    label: "Duration"    },
  { value: "date_added",     label: "Date added"  },
  { value: "last_played_at", label: "Last played" },
];

// ---------------------------------------------------------------------------
// Rule row component
// ---------------------------------------------------------------------------

type DraftRule = Omit<SmartPlaylistRule, "id" | "playlist_id" | "position"> & { _key: number };

function RuleRow({
  rule,
  onChange,
  onDelete,
}: {
  rule: DraftRule;
  onChange: (r: DraftRule) => void;
  onDelete: () => void;
}) {
  const ops = operatorsForField(rule.field);

  return (
    <div className="flex items-center gap-2 flex-wrap">
      <select
        value={rule.field}
        onChange={(e) => {
          const field = e.target.value;
          const operator = defaultOperator(field);
          onChange({ ...rule, field, operator, value: "" });
        }}
        className="rounded-md border border-input bg-background px-2 py-1 text-sm"
      >
        {FIELDS.map((f) => (
          <option key={f.value} value={f.value}>{f.label}</option>
        ))}
      </select>

      <select
        value={rule.operator}
        onChange={(e) => onChange({ ...rule, operator: e.target.value })}
        className="rounded-md border border-input bg-background px-2 py-1 text-sm"
      >
        {ops.map((o) => (
          <option key={o.value} value={o.value}>{o.label}</option>
        ))}
      </select>

      <input
        type="text"
        value={rule.value}
        onChange={(e) => onChange({ ...rule, value: e.target.value })}
        placeholder="value"
        className="rounded-md border border-input bg-background px-2 py-1 text-sm w-28"
      />

      <button
        onClick={onDelete}
        className="text-muted-foreground hover:text-destructive transition-colors"
        aria-label="Remove rule"
      >
        <Trash2 size={14} />
      </button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main page
// ---------------------------------------------------------------------------

let _keyCounter = 0;
const nextKey = () => ++_keyCounter;

function ruleToD(r: SmartPlaylistRule): DraftRule {
  return { _key: nextKey(), field: r.field, operator: r.operator, value: r.value };
}

export default function SmartPlaylistDetail() {
  const { id } = useParams<{ id: string }>();
  const playlistId = Number(id);
  const navigate = useNavigate();

  const [playlist, setPlaylist] = useState<SmartPlaylist | null>(null);
  const [tracks, setTracks] = useState<SmartTrack[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);

  // Editor state
  const [draftName, setDraftName] = useState("");
  const [draftMatch, setDraftMatch] = useState<"all" | "any">("all");
  const [draftRules, setDraftRules] = useState<DraftRule[]>([]);
  const [draftSortField, setDraftSortField] = useState("title");
  const [draftSortDir, setDraftSortDir] = useState<"asc" | "desc">("asc");
  const [draftLimit, setDraftLimit] = useState<string>("");
  const [saving, setSaving] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const isBuiltin = BUILTIN_IDS.has(playlistId);
  const libraryTick = useLibraryRefresh();

  const loadData = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [pl, rules] = await getSmartPlaylist(playlistId);
      setPlaylist(pl);
      setDraftName(pl.name);
      setDraftMatch(pl.match_mode);
      setDraftRules(rules.map(ruleToD));
      setDraftSortField(pl.sort_field);
      setDraftSortDir(pl.sort_direction);
      setDraftLimit(pl.limit_count != null ? String(pl.limit_count) : "");

      const t = await getSmartPlaylistTracks(playlistId);
      setTracks(t);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [playlistId]);

  useEffect(() => { loadData(); }, [loadData]);

  // Auto-refresh the matched tracks when the library mutates (scan, metadata
  // edit, play-count change). Only the track list is refetched — the playlist
  // rules / edit draft are left untouched so an in-progress edit isn't clobbered.
  // Skipped on the initial mount (libraryTick === 0), since loadData already ran.
  useEffect(() => {
    if (libraryTick === 0) return;
    let cancelled = false;
    getSmartPlaylistTracks(playlistId)
      .then((t) => { if (!cancelled) setTracks(t); })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [libraryTick, playlistId]);

  const handleSave = async () => {
    if (!playlist) return;
    setSaving(true);
    try {
      const limitVal = draftLimit.trim() === "" ? null : parseInt(draftLimit, 10);
      const updated = await updateSmartPlaylist(
        playlistId,
        draftName,
        draftMatch,
        draftSortField,
        draftSortDir,
        limitVal && !isNaN(limitVal) ? limitVal : null,
      );
      await setSmartPlaylistRules(
        playlistId,
        draftRules.map((r, i) => ({
          id: 0,
          playlist_id: playlistId,
          field: r.field,
          operator: r.operator,
          value: r.value,
          position: i,
        })),
      );
      setPlaylist(updated);
      setEditing(false);
      const t = await getSmartPlaylistTracks(playlistId);
      setTracks(t);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async () => {
    try {
      await deleteSmartPlaylist(playlistId);
      navigate("/playlists");
    } catch (e) {
      setError(String(e));
    }
  };

  const handlePlay = async (startIndex = 0) => {
    if (tracks.length === 0) return;
    try {
      await setQueueAndPlay(tracks.map((t) => t.id), startIndex);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleShuffle = async () => {
    if (tracks.length === 0) return;
    const idx = Math.floor(Math.random() * tracks.length);
    handlePlay(idx);
  };

  const addRule = () => {
    setDraftRules((prev) => [
      ...prev,
      { _key: nextKey(), field: "title", operator: "contains", value: "" },
    ]);
  };

  if (loading) return <div className="p-6 text-sm text-muted-foreground">Loading…</div>;
  if (!playlist) return <div className="p-6 text-sm text-destructive">{error ?? "Not found"}</div>;

  return (
    <div className="p-6 max-w-3xl">
      {/* Header */}
      <div className="flex items-center gap-3 mb-6">
        <button
          onClick={() => navigate("/playlists")}
          className="text-muted-foreground hover:text-foreground transition-colors"
          aria-label="Back"
        >
          <ChevronLeft size={20} />
        </button>
        <div className="flex-1 min-w-0">
          <h1 className="text-2xl font-semibold truncate">{playlist.name}</h1>
          <p className="text-sm text-muted-foreground">
            {tracks.length} {tracks.length === 1 ? "track" : "tracks"}
            {playlist.limit_count != null && ` · limit ${playlist.limit_count}`}
          </p>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <button
            onClick={() => loadData()}
            className="p-1.5 rounded-md text-muted-foreground hover:text-foreground transition-colors"
            aria-label="Refresh"
          >
            <RefreshCw size={16} />
          </button>
          {!isBuiltin && (
            <button
              onClick={() => setEditing((v) => !v)}
              className={cn(
                "rounded-md border px-3 py-1 text-xs transition-colors",
                editing
                  ? "border-primary text-primary"
                  : "border-border hover:bg-muted/40",
              )}
            >
              {editing ? "Cancel" : "Edit rules"}
            </button>
          )}
          {isBuiltin && (
            <button
              onClick={() => setEditing((v) => !v)}
              className={cn(
                "rounded-md border px-3 py-1 text-xs transition-colors",
                editing
                  ? "border-primary text-primary"
                  : "border-border hover:bg-muted/40",
              )}
            >
              {editing ? "Close" : "View rules"}
            </button>
          )}
        </div>
      </div>

      {error && <p className="text-sm text-destructive mb-4">{error}</p>}

      {/* Rule editor */}
      {editing && (
        <div className="rounded-lg border border-border p-4 mb-6 space-y-4">
          {/* Name + match mode */}
          <div className="flex flex-wrap items-center gap-3">
            <input
              value={draftName}
              onChange={(e) => setDraftName(e.target.value)}
              disabled={isBuiltin}
              className="rounded-md border border-input bg-background px-3 py-1 text-sm flex-1 min-w-32 disabled:opacity-60"
              placeholder="Playlist name"
            />
            <div className="flex items-center gap-2 text-sm">
              <span className="text-muted-foreground">Match</span>
              {(["all", "any"] as const).map((m) => (
                <button
                  key={m}
                  disabled={isBuiltin}
                  onClick={() => setDraftMatch(m)}
                  className={cn(
                    "px-2.5 py-1 rounded-md border text-xs transition-colors disabled:opacity-60",
                    draftMatch === m
                      ? "bg-primary text-primary-foreground border-primary"
                      : "border-border hover:bg-muted/40",
                  )}
                >
                  {m === "all" ? "All rules" : "Any rule"}
                </button>
              ))}
            </div>
          </div>

          {/* Rules list */}
          <div className="space-y-2">
            {draftRules.map((rule) => (
              <RuleRow
                key={rule._key}
                rule={rule}
                onChange={(r) =>
                  setDraftRules((prev) => prev.map((x) => (x._key === rule._key ? r : x)))
                }
                onDelete={() =>
                  setDraftRules((prev) => prev.filter((x) => x._key !== rule._key))
                }
              />
            ))}
            {draftRules.length === 0 && (
              <p className="text-sm text-muted-foreground">No rules — all tracks will match.</p>
            )}
          </div>

          {!isBuiltin && (
            <button
              onClick={addRule}
              className="inline-flex items-center gap-1.5 text-sm text-muted-foreground hover:text-foreground transition-colors"
            >
              <Plus size={14} /> Add rule
            </button>
          )}

          {/* Sort + limit */}
          <div className="flex flex-wrap items-center gap-3 pt-2 border-t border-border">
            <span className="text-sm text-muted-foreground">Sort by</span>
            <select
              value={draftSortField}
              disabled={isBuiltin}
              onChange={(e) => setDraftSortField(e.target.value)}
              className="rounded-md border border-input bg-background px-2 py-1 text-sm disabled:opacity-60"
            >
              {SORT_FIELDS.map((f) => (
                <option key={f.value} value={f.value}>{f.label}</option>
              ))}
            </select>
            <select
              value={draftSortDir}
              disabled={isBuiltin}
              onChange={(e) => setDraftSortDir(e.target.value as "asc" | "desc")}
              className="rounded-md border border-input bg-background px-2 py-1 text-sm disabled:opacity-60"
            >
              <option value="asc">Ascending</option>
              <option value="desc">Descending</option>
            </select>
            <span className="text-sm text-muted-foreground">Limit</span>
            <input
              type="number"
              value={draftLimit}
              disabled={isBuiltin}
              onChange={(e) => setDraftLimit(e.target.value)}
              placeholder="None"
              min={1}
              max={10000}
              className="rounded-md border border-input bg-background px-2 py-1 text-sm w-20 disabled:opacity-60"
            />
          </div>

          {!isBuiltin && (
            <div className="flex items-center gap-3 pt-2 border-t border-border">
              <button
                onClick={handleSave}
                disabled={saving || !draftName.trim()}
                className="rounded-md bg-primary text-primary-foreground px-4 py-1.5 text-sm disabled:opacity-50"
              >
                {saving ? "Saving…" : "Save"}
              </button>
              {!confirmDelete ? (
                <button
                  onClick={() => setConfirmDelete(true)}
                  className="text-sm text-muted-foreground hover:text-destructive transition-colors"
                >
                  Delete playlist
                </button>
              ) : (
                <div className="flex items-center gap-2">
                  <span className="text-sm text-destructive">Delete?</span>
                  <button
                    onClick={handleDelete}
                    className="text-sm text-destructive hover:underline"
                  >
                    Yes
                  </button>
                  <button
                    onClick={() => setConfirmDelete(false)}
                    className="text-sm text-muted-foreground hover:underline"
                  >
                    Cancel
                  </button>
                </div>
              )}
            </div>
          )}
        </div>
      )}

      {/* Playback buttons */}
      {tracks.length > 0 && (
        <div className="flex items-center gap-2 mb-4">
          <button
            onClick={() => handlePlay(0)}
            className="inline-flex items-center gap-1.5 rounded-md bg-primary text-primary-foreground px-3 py-1.5 text-sm hover:opacity-90"
          >
            <Play size={14} fill="currentColor" />
            Play all
          </button>
          <button
            onClick={handleShuffle}
            className="inline-flex items-center gap-1.5 rounded-md border border-border px-3 py-1.5 text-sm hover:bg-muted/40 transition-colors"
          >
            <Shuffle size={14} />
            Shuffle
          </button>
        </div>
      )}

      {/* Track list */}
      {tracks.length === 0 ? (
        <p className="text-sm text-muted-foreground">No tracks match this playlist's rules.</p>
      ) : (
        <div className="space-y-0.5">
          {tracks.map((track, idx) => (
            <button
              key={track.id}
              onDoubleClick={() => handlePlay(idx)}
              className="w-full flex items-center gap-3 rounded-md px-2 py-1.5 hover:bg-muted/30 transition-colors group text-left"
            >
              <AlbumArt
                path={track.album_art_path}
                size={36}
                className="rounded shrink-0"
              />
              <div className="flex-1 min-w-0">
                <p className="text-sm truncate">{track.title ?? track.file_path.split(/[\\/]/).pop()}</p>
                <p className="text-xs text-muted-foreground truncate">
                  {track.artists.join(", ")}
                  {track.album_name && ` · ${track.album_name}`}
                </p>
              </div>
              <span className="text-xs text-muted-foreground tabular-nums shrink-0">
                {track.duration_ms != null ? formatDuration(track.duration_ms) : "—"}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
