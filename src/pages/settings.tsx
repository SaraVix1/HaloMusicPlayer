import { useEffect, useLayoutEffect, useState, useCallback, useRef } from "react";
import { useSearchParams } from "react-router-dom";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  getFolders,
  addFolder,
  removeFolder,
  resetAllStats,
  scanLibrary,
  clearCache,
  clearDatabase,
  getScanSettings,
  setScanSettings,
  getCrossfadeMs,
  setCrossfadeMs,
  getEq,
  setEqBand,
  setEqBypass,
  setEqPreset,
  setEqStereo,
  setEqDynamic,
  STEREO_WIDTH_MAX,
  listUserPresets,
  saveUserPreset,
  loadUserPreset,
  deleteUserPreset,
  getCurrentDevice,
  saveDeviceEqProfile,
  deleteDeviceEqProfile,
  lastfmGetStatus,
  lastfmSetCredentials,
  lastfmStartAuth,
  lastfmCompleteAuth,
  lastfmLogout,
  getPref,
  setPref,
  setWatchFolders,
  matchEqPreset,
  EQ_BAND_LABELS,
  EQ_PRESETS,
  type Folder,
  type ScanProgress,
  type ScanSummary,
  type ScanSettings,
  type EqConfig,
  type UserPreset,
  type DeviceEqProfile,
  type LastFmStatus,
} from "@/lib/ipc";
import { Slider } from "@/components/ui/slider";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  FolderOpen,
  X,
  RefreshCw,
  Trash2,
  Eraser,
  Monitor,
  Moon,
  Sun,
  BarChart2,
  Settings2,
  Palette,
  Play,
  SlidersHorizontal,
  LibraryBig,
  Wrench,
} from "lucide-react";
import { useThemeStore } from "@/lib/theme-store";
import { getLastTab, setLastTab } from "@/lib/tab-memory";
import type { Theme } from "@/lib/ipc";
import { cn } from "@/lib/utils";

// ── Types ──────────────────────────────────────────────────────────────────────

type Category =
  | "general"
  | "appearance"
  | "playback"
  | "equalizer"
  | "library"
  | "advanced";

const CATEGORIES: { id: Category; label: string; icon: React.ElementType }[] = [
  { id: "general",    label: "General",    icon: Settings2 },
  { id: "appearance", label: "Appearance", icon: Palette },
  { id: "playback",   label: "Playback",   icon: Play },
  { id: "equalizer",  label: "Equalizer",  icon: SlidersHorizontal },
  { id: "library",    label: "Library",    icon: LibraryBig },
  { id: "advanced",   label: "Advanced",   icon: Wrench },
];

// ── Small layout helpers ───────────────────────────────────────────────────────

function SectionHeader({ title, description }: { title: string; description?: string }) {
  return (
    <div className="mb-6">
      <h2 className="text-base font-semibold">{title}</h2>
      {description && (
        <p className="text-sm text-muted-foreground mt-0.5">{description}</p>
      )}
      <Separator className="mt-3" />
    </div>
  );
}

function SettingRow({
  label,
  description,
  children,
}: {
  label: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-6 py-3 border-b border-border/50 last:border-0">
      <div className="min-w-0">
        <div className="text-sm font-medium">{label}</div>
        {description && (
          <div className="text-xs text-muted-foreground mt-0.5 leading-relaxed">
            {description}
          </div>
        )}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function Toggle({
  checked,
  onChange,
  disabled,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <button
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn(
        "relative inline-flex h-5 w-9 shrink-0 rounded-full border-2 border-transparent transition-colors focus-visible:outline-none",
        checked ? "bg-primary" : "bg-muted",
        disabled && "opacity-40 pointer-events-none",
      )}
    >
      <span
        className={cn(
          "pointer-events-none block h-4 w-4 rounded-full bg-white shadow-sm transition-transform",
          checked ? "translate-x-4" : "translate-x-0",
        )}
      />
    </button>
  );
}

// ── Section: General ───────────────────────────────────────────────────────────

function GeneralSection({
  closeBehavior,
  onCloseBehaviorChange,
  resumeOnLaunch,
  onResumeOnLaunchChange,
}: {
  closeBehavior: string;
  onCloseBehaviorChange: (v: string) => void;
  resumeOnLaunch: boolean;
  onResumeOnLaunchChange: (v: boolean) => void;
}) {
  return (
    <div>
      <SectionHeader
        title="General"
        description="Startup behaviour and window management."
      />

      <SettingRow
        label="Close button"
        description="What happens when you click the × in the title bar."
      >
        <select
          value={closeBehavior}
          onChange={(e) => onCloseBehaviorChange(e.target.value)}
          className="rounded-md border border-input bg-background px-2 py-1 text-sm min-w-[160px]"
        >
          <option value="minimize">Minimize to tray</option>
          <option value="quit">Quit Halo</option>
        </select>
      </SettingRow>

      <SettingRow
        label="Resume last session on launch"
        description="Continue playback from where you left off when Halo starts."
      >
        <Toggle checked={resumeOnLaunch} onChange={onResumeOnLaunchChange} />
      </SettingRow>
    </div>
  );
}

// ── Section: Appearance ────────────────────────────────────────────────────────

function AppearanceSection({
  theme,
  onThemeChange,
}: {
  theme: Theme;
  onThemeChange: (t: Theme) => void;
}) {
  const themeOptions: { value: Theme; label: string; icon: React.ElementType }[] = [
    { value: "light",  label: "Light",  icon: Sun },
    { value: "dark",   label: "Dark",   icon: Moon },
    { value: "system", label: "System", icon: Monitor },
  ];

  return (
    <div>
      <SectionHeader title="Appearance" description="Colour scheme and visual style." />

      <SettingRow label="Theme" description="Choose how Halo looks.">
        <div className="inline-flex rounded-md border border-border p-0.5">
          {themeOptions.map((opt) => {
            const Icon = opt.icon;
            const active = theme === opt.value;
            return (
              <button
                key={opt.value}
                onClick={() => onThemeChange(opt.value)}
                className={cn(
                  "inline-flex items-center gap-1.5 rounded px-3 py-1.5 text-sm transition-colors",
                  active
                    ? "bg-muted text-foreground"
                    : "text-muted-foreground hover:text-foreground",
                )}
              >
                <Icon size={14} />
                {opt.label}
              </button>
            );
          })}
        </div>
      </SettingRow>
    </div>
  );
}

// ── Section: Playback ──────────────────────────────────────────────────────────

function PlaybackSection({
  crossfadeMs,
  onCrossfadeChange,
  onCrossfadeCommit,
}: {
  crossfadeMs: number;
  onCrossfadeChange: (ms: number) => void;
  onCrossfadeCommit: (ms: number) => void;
}) {
  return (
    <div>
      <SectionHeader title="Playback" description="Audio playback and transition settings." />

      <SettingRow
        label="Crossfade"
        description="Fade between consecutive tracks. Set to 0 to disable."
      >
        <div className="flex items-center gap-3 w-56">
          <Slider
            value={[crossfadeMs]}
            min={0}
            max={12000}
            step={500}
            onValueChange={(v) => onCrossfadeChange(Array.isArray(v) ? v[0] : (v as number))}
            onValueCommitted={(v) => onCrossfadeCommit(Array.isArray(v) ? v[0] : (v as number))}
            className="flex-1"
            aria-label="Crossfade duration"
          />
          <span className="text-sm text-muted-foreground tabular-nums w-10 text-right">
            {(crossfadeMs / 1000).toFixed(1)}s
          </span>
        </div>
      </SettingRow>
    </div>
  );
}

// ── Section: Equalizer ─────────────────────────────────────────────────────────

function EqualizerSection({
  eq,
  userPresets,
  currentDevice,
  savingDeviceProfile,
  savePresetName,
  savingPreset,
  onBypassToggle,
  onPreset,
  onLoadUserPreset,
  onSaveUserPreset,
  onDeleteUserPreset,
  onBandChange,
  onBandCommit,
  onSaveDeviceProfile,
  onDeleteDeviceProfile,
  onSavePresetNameChange,
  onStereoToggle,
  onStereoWidthChange,
  onStereoWidthCommit,
  onDynamicToggle,
}: {
  eq: EqConfig;
  userPresets: UserPreset[];
  currentDevice: DeviceEqProfile | null;
  savingDeviceProfile: boolean;
  savePresetName: string;
  savingPreset: boolean;
  onBypassToggle: () => void;
  onPreset: (p: string) => void;
  onLoadUserPreset: (id: number) => void;
  onSaveUserPreset: () => void;
  onDeleteUserPreset: (id: number) => void;
  onBandChange: (band: number, gain: number) => void;
  onBandCommit: (band: number, gain: number) => void;
  onSaveDeviceProfile: () => void;
  onDeleteDeviceProfile: () => void;
  onSavePresetNameChange: (name: string) => void;
  onStereoToggle: (enabled: boolean) => void;
  onStereoWidthChange: (width: number) => void;
  onStereoWidthCommit: (width: number) => void;
  onDynamicToggle: (enabled: boolean) => void;
}) {
  const activePreset = matchEqPreset(eq.bands);
  const isFlat = eq.bands.every((g) => g === 0);
  const activeUserPresetId = userPresets.find((p) =>
    p.bands.every((v, i) => Math.abs(v - eq.bands[i]) < 0.05),
  )?.id;

  return (
    <div>
      <div className="mb-6">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-base font-semibold">Equalizer</h2>
            <p className="text-sm text-muted-foreground mt-0.5">
              10-band graphic equalizer.{" "}
              <span className="text-foreground/70">
                Current: {activePreset ?? "Custom"}
              </span>
            </p>
          </div>
          <label className="flex items-center gap-2 text-sm text-muted-foreground cursor-pointer select-none">
            <span>{eq.bypass ? "Bypassed" : "Active"}</span>
            <Toggle checked={!eq.bypass} onChange={() => onBypassToggle()} />
          </label>
        </div>
        <Separator className="mt-3" />
      </div>

      {currentDevice && (
        <div className="flex items-center gap-3 mb-5 rounded-md border border-border bg-muted/30 px-3 py-2 text-sm">
          <span className="text-muted-foreground shrink-0">Device</span>
          <span className="font-medium truncate flex-1">{currentDevice.device_name}</span>
          <Button
            variant="outline"
            size="sm"
            onClick={onSaveDeviceProfile}
            disabled={savingDeviceProfile}
            className="shrink-0"
          >
            {savingDeviceProfile ? "Saving…" : "Save profile"}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={onDeleteDeviceProfile}
            className="shrink-0 text-muted-foreground hover:text-destructive"
            title="Remove device profile"
          >
            <X size={14} />
          </Button>
        </div>
      )}

      {/* Built-in presets as selectable chips — the active one is highlighted */}
      <div className="mb-5">
        <div className="flex items-center justify-between mb-2">
          <span className="text-sm font-medium">Presets</span>
          <button
            onClick={() => onPreset("Flat")}
            disabled={isFlat}
            className={cn(
              "text-xs transition-colors",
              isFlat
                ? "text-muted-foreground/40 cursor-default"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            Reset to flat
          </button>
        </div>
        <div className="flex flex-wrap gap-1.5">
          {EQ_PRESETS.map((p) => {
            const active = p === activePreset;
            return (
              <button
                key={p}
                onClick={() => onPreset(p)}
                className={cn(
                  "rounded-full border px-3 py-1 text-sm transition-colors",
                  active
                    ? "border-primary bg-primary text-primary-foreground"
                    : "border-border bg-muted/40 text-muted-foreground hover:text-foreground hover:border-foreground/30",
                )}
              >
                {p}
              </button>
            );
          })}
        </div>
      </div>

      {/* User presets */}
      <div className="mb-6">
        <span className="text-sm font-medium block mb-2">My Presets</span>
        {userPresets.length > 0 && (
          <div className="flex flex-wrap items-center gap-1.5 mb-3">
            {userPresets.map((p) => {
              const active = p.id === activeUserPresetId;
              return (
                <div
                  key={p.id}
                  className={cn(
                    "flex items-center gap-1 rounded-full border pl-3 pr-1.5 py-0.5 text-sm transition-colors",
                    active
                      ? "border-primary bg-primary text-primary-foreground"
                      : "border-border bg-muted/40",
                  )}
                >
                  <button
                    onClick={() => onLoadUserPreset(p.id)}
                    className={cn(
                      "transition-colors",
                      active
                        ? "text-primary-foreground"
                        : "text-muted-foreground hover:text-foreground",
                    )}
                  >
                    {p.name}
                  </button>
                  <button
                    onClick={() => onDeleteUserPreset(p.id)}
                    className={cn(
                      "transition-colors ml-1",
                      active
                        ? "text-primary-foreground/80 hover:text-primary-foreground"
                        : "text-muted-foreground hover:text-destructive",
                    )}
                    aria-label={`Delete preset ${p.name}`}
                  >
                    <X size={12} />
                  </button>
                </div>
              );
            })}
          </div>
        )}
        <div className="flex items-center gap-1.5">
          <input
            type="text"
            value={savePresetName}
            onChange={(e) => onSavePresetNameChange(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") onSaveUserPreset(); }}
            placeholder="Save current curve as…"
            className="rounded-md border border-input bg-background px-3 py-1 text-sm w-44"
          />
          <Button
            variant="outline"
            size="sm"
            onClick={onSaveUserPreset}
            disabled={!savePresetName.trim() || savingPreset}
          >
            Save
          </Button>
        </div>
      </div>

      {/* Band sliders with a dB scale */}
      <div className={cn("flex gap-3", eq.bypass && "opacity-50 pointer-events-none")}>
        {/* dB axis */}
        <div className="flex flex-col items-end shrink-0 select-none" style={{ width: 32 }}>
          <span style={{ minHeight: 16 }} />
          <div className="flex flex-col justify-between items-end text-[10px] text-muted-foreground tabular-nums" style={{ height: 120 }}>
            <span>+12</span>
            <span>+6</span>
            <span>0</span>
            <span>−6</span>
            <span>−12</span>
          </div>
          <span className="text-xs">&nbsp;</span>
        </div>

        {eq.bands.map((gain, i) => {
          const fmt = gain === 0 ? "0" : gain > 0 ? `+${gain.toFixed(1)}` : gain.toFixed(1);
          return (
            <div key={i} className="flex flex-col items-center gap-1" style={{ width: 36 }}>
              <span
                className={cn(
                  "text-xs tabular-nums",
                  gain === 0 ? "text-muted-foreground" : "text-foreground",
                )}
                style={{ minHeight: 16 }}
              >
                {fmt}
              </span>
              <div style={{ height: 120 }}>
                <Slider
                  orientation="vertical"
                  min={-12}
                  max={12}
                  step={0.5}
                  value={[gain]}
                  onValueChange={(v) => onBandChange(i, Array.isArray(v) ? v[0] : (v as number))}
                  onValueCommitted={(v) => onBandCommit(i, Array.isArray(v) ? v[0] : (v as number))}
                  className="h-full"
                  aria-label={`${EQ_BAND_LABELS[i]}Hz gain`}
                />
              </div>
              <span className="text-xs text-muted-foreground">{EQ_BAND_LABELS[i]}</span>
            </div>
          );
        })}
      </div>

      {/* Sound enhancements */}
      <Separator className="my-6" />
      <div className="text-sm font-medium mb-1">Sound enhancements</div>

      <SettingRow
        label="Stereo expander"
        description="Widen the stereo image for a more spacious sound."
      >
        <Toggle checked={eq.stereo} onChange={onStereoToggle} />
      </SettingRow>

      {eq.stereo && (
        <SettingRow
          label="Width"
          description="Higher values push the stereo field wider. Neutral is 1.0×."
        >
          <div className="flex items-center gap-3 w-56">
            <Slider
              value={[eq.stereo_width]}
              min={0}
              max={STEREO_WIDTH_MAX}
              step={0.1}
              onValueChange={(v) => onStereoWidthChange(Array.isArray(v) ? v[0] : (v as number))}
              onValueCommitted={(v) => onStereoWidthCommit(Array.isArray(v) ? v[0] : (v as number))}
              className="flex-1"
              aria-label="Stereo width"
            />
            <span className="text-sm text-muted-foreground tabular-nums w-10 text-right">
              {eq.stereo_width.toFixed(1)}×
            </span>
          </div>
        </SettingRow>
      )}

      <SettingRow
        label="Dynamic EQ"
        description="Eases the EQ effect on loud passages and applies it fully when quiet, so boosts stay clean."
      >
        <Toggle checked={eq.dynamic} onChange={onDynamicToggle} />
      </SettingRow>
    </div>
  );
}

// ── Section: Library ───────────────────────────────────────────────────────────

function LibrarySection({
  folders,
  loading,
  scanning,
  overrideMetadata,
  progress,
  autoScanOnAdd,
  watchFolders,
  scanSettings,
  onAddFolder,
  onRemoveFolder,
  onScan,
  onOverrideMetadataChange,
  onAutoScanOnAddChange,
  onWatchFoldersChange,
  onScanSettingsChange,
}: {
  folders: Folder[];
  loading: boolean;
  scanning: boolean;
  overrideMetadata: boolean;
  progress: ScanProgress | null;
  autoScanOnAdd: boolean;
  watchFolders: boolean;
  scanSettings: ScanSettings;
  onAddFolder: () => void;
  onRemoveFolder: (id: number) => void;
  onScan: () => void;
  onOverrideMetadataChange: (v: boolean) => void;
  onAutoScanOnAddChange: (v: boolean) => void;
  onWatchFoldersChange: (v: boolean) => void;
  onScanSettingsChange: (s: ScanSettings) => void;
}) {
  const progressPct =
    progress && progress.total > 0
      ? Math.round((progress.current / progress.total) * 100)
      : 0;

  return (
    <div>
      <SectionHeader
        title="Library"
        description="Music folders, scan options, and file format settings."
      />

      {/* Folders */}
      <div className="mb-6">
        <div className="text-sm font-medium mb-3">Music folders</div>
        <div className="flex items-center gap-3 mb-3 flex-wrap">
          <Button onClick={onAddFolder} variant="outline" size="sm">
            <FolderOpen size={15} className="mr-2" />
            Add folder
          </Button>
          <Button
            variant="outline"
            size="sm"
            disabled={folders.length === 0 || scanning}
            onClick={onScan}
          >
            <RefreshCw size={15} className={cn("mr-2", scanning && "animate-spin")} />
            {scanning ? "Scanning…" : "Rescan library"}
          </Button>
        </div>

        {loading ? (
          <p className="text-sm text-muted-foreground">Loading…</p>
        ) : folders.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            No folders added yet. Click "Add folder" to get started.
          </p>
        ) : (
          <ul className="space-y-1">
            {folders.map((folder) => (
              <li
                key={folder.id}
                className="flex items-center justify-between gap-3 rounded-md px-3 py-2 bg-muted/40 group"
              >
                <span className="text-sm font-mono truncate">{folder.path}</span>
                <button
                  onClick={() => onRemoveFolder(folder.id)}
                  className="text-muted-foreground hover:text-destructive transition-colors shrink-0 opacity-0 group-hover:opacity-100"
                  aria-label="Remove folder"
                  disabled={scanning}
                >
                  <X size={15} />
                </button>
              </li>
            ))}
          </ul>
        )}

        {progress && (
          <div className="mt-4 rounded-md bg-muted/40 px-3 py-3">
            <div className="flex items-center justify-between mb-2">
              <span className="text-sm font-medium">
                {progress.done ? "Scan complete" : "Scanning…"}
              </span>
              <span className="text-xs text-muted-foreground tabular-nums">
                {progress.current} / {progress.total} ({progressPct}%)
              </span>
            </div>
            <div className="h-1.5 w-full bg-muted rounded overflow-hidden">
              <div
                className="h-full bg-primary transition-[width] duration-150"
                style={{ width: `${progressPct}%` }}
              />
            </div>
            {progress.current_file && !progress.done && (
              <p className="text-xs text-muted-foreground mt-2 font-mono truncate">
                {progress.current_file}
              </p>
            )}
            <p className="text-xs text-muted-foreground mt-2">
              Inserted {progress.inserted} · Updated {progress.updated} ·
              Skipped {progress.skipped} · Failed {progress.failed}
            </p>
          </div>
        )}
      </div>

      <Separator className="my-5" />

      {/* Scan behaviour settings */}
      <div className="mb-6">
        <div className="text-sm font-medium mb-3">Scan behaviour</div>

        <SettingRow
          label="Override existing metadata"
          description="Re-read tags from files even when metadata is already in the database."
        >
          <Toggle checked={overrideMetadata} onChange={onOverrideMetadataChange} disabled={scanning} />
        </SettingRow>

        <SettingRow
          label="Auto-scan when adding a folder"
          description="Start a library scan automatically after a new folder is added."
        >
          <Toggle checked={autoScanOnAdd} onChange={onAutoScanOnAddChange} />
        </SettingRow>

        <SettingRow
          label="Watch folders for changes"
          description="Automatically update the library when files are added or removed."
        >
          <Toggle checked={watchFolders} onChange={onWatchFoldersChange} />
        </SettingRow>
      </div>

      <Separator className="my-5" />

      {/* Format settings */}
      <div className="space-y-4">
        <div className="text-sm font-medium mb-1">File formats</div>
        <div>
          <label className="text-sm font-medium block mb-1.5">Multi-value delimiters</label>
          <input
            type="text"
            value={scanSettings.delimiters}
            onChange={(e) => onScanSettingsChange({ ...scanSettings, delimiters: e.target.value })}
            className="w-full rounded-md border border-input bg-background px-3 py-1.5 text-sm font-mono"
            placeholder=",;|:&"
          />
          <p className="text-xs text-muted-foreground mt-1">
            Characters used to split fields like artist, genre, and composer.
          </p>
        </div>
        <div>
          <label className="text-sm font-medium block mb-1.5">File extensions</label>
          <input
            type="text"
            value={scanSettings.extensions}
            onChange={(e) => onScanSettingsChange({ ...scanSettings, extensions: e.target.value })}
            className="w-full rounded-md border border-input bg-background px-3 py-1.5 text-sm font-mono"
            placeholder="mp3,flac,m4a"
          />
          <p className="text-xs text-muted-foreground mt-1">
            Comma-separated list, without dots.
          </p>
        </div>
      </div>
    </div>
  );
}

// ── Section: Last.fm ────────────────────────────────────────────────────────────

// Self-contained Last.fm panel inside the Advanced section. The app API key +
// shared secret are entered here once (Settings → Advanced) and persisted; each
// person then authorises their own account in a browser — that per-user session
// is memory-only. Manages its own status/auth state so the parent stays simple.
function LastFmPanel() {
  const [status, setStatus] = useState<LastFmStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Credentials form (API key + shared secret).
  const [editingCreds, setEditingCreds] = useState(false);
  const [keyInput, setKeyInput] = useState("");
  const [secretInput, setSecretInput] = useState("");

  // "Authenticate" step.
  const [authUrl, setAuthUrl] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const refresh = async () => setStatus(await lastfmGetStatus());

  useEffect(() => {
    lastfmGetStatus().then(setStatus).catch(() => {});
  }, []);

  // While auth is pending, poll complete-auth so the connection finishes
  // hands-free once the user clicks Allow. complete-auth errors until the token
  // is approved, so failures are swallowed; success flips pending→connected,
  // changing this effect's deps and stopping the poll.
  useEffect(() => {
    if (!status?.pending) return;
    let cancelled = false;
    const interval = setInterval(async () => {
      try {
        await lastfmCompleteAuth();
        if (!cancelled) setStatus(await lastfmGetStatus());
      } catch {
        // Token not authorised yet — keep waiting.
      }
    }, 3000);
    const timeout = setTimeout(() => clearInterval(interval), 120_000);
    return () => {
      cancelled = true;
      clearInterval(interval);
      clearTimeout(timeout);
    };
  }, [status?.pending]);

  const handleSaveCreds = async () => {
    setBusy(true);
    setError(null);
    try {
      await lastfmSetCredentials(keyInput.trim(), secretInput.trim());
      setKeyInput("");
      setSecretInput("");
      setEditingCreds(false);
      setAuthUrl(null);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  // The auth URL for this attempt — cached from "Connect", or regenerated lazily
  // (e.g. after the panel was remounted while still pending).
  const getAuthUrl = async (): Promise<string> => {
    if (authUrl) return authUrl;
    const u = await lastfmStartAuth();
    setAuthUrl(u);
    return u;
  };

  // "Connect with browser" → fetch a token and enter the Authenticate step
  // (which offers Open in browser / Copy link). Does not auto-open the browser.
  const handleConnect = async () => {
    setBusy(true);
    setError(null);
    try {
      const u = await lastfmStartAuth();
      setAuthUrl(u);
      await refresh(); // status now pending → shows the two options
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleOpenInBrowser = async () => {
    setError(null);
    try {
      await openUrl(await getAuthUrl());
    } catch (e) {
      setError(String(e));
    }
  };

  const handleCopyLink = async () => {
    setError(null);
    try {
      const u = await getAuthUrl();
      await navigator.clipboard.writeText(u);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleCompleteAuth = async () => {
    setBusy(true);
    setError(null);
    try {
      await lastfmCompleteAuth();
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleDisconnect = async () => {
    setBusy(true);
    setError(null);
    try {
      await lastfmLogout();
      setAuthUrl(null);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const configured = status?.configured ?? false;
  const showCredsForm = !configured || editingCreds;

  return (
    <div>
      <div className="text-sm font-medium mb-1">Last.fm scrobbling</div>
      <p className="text-xs text-muted-foreground mb-4 max-w-md">
        Optional. Scrobbles the tracks you play to your Last.fm profile. Enter the
        app’s API key once, then each person signs in to their own account —
        sign-in clears when the app closes.
      </p>

      {showCredsForm ? (
        <div className="space-y-3 max-w-md">
          <p className="text-xs text-muted-foreground">
            Create an API account at{" "}
            <button
              className="underline hover:text-foreground"
              onClick={() => openUrl("https://www.last.fm/api/account/create")}
            >
              last.fm/api/account/create
            </button>{" "}
            and paste the key and shared secret here.
          </p>
          <div>
            <label className="text-xs font-medium block mb-1">API key</label>
            <input
              type="text"
              value={keyInput}
              onChange={(e) => setKeyInput(e.target.value)}
              className="w-full rounded-md border border-input bg-background px-3 py-1.5 text-sm font-mono"
              placeholder="32-character API key"
            />
          </div>
          <div>
            <label className="text-xs font-medium block mb-1">Shared secret</label>
            <input
              type="password"
              value={secretInput}
              onChange={(e) => setSecretInput(e.target.value)}
              className="w-full rounded-md border border-input bg-background px-3 py-1.5 text-sm font-mono"
              placeholder="Shared secret"
            />
          </div>
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              disabled={busy || !keyInput.trim() || !secretInput.trim()}
              onClick={handleSaveCreds}
            >
              {busy ? "Saving…" : "Save"}
            </Button>
            {configured && (
              <Button variant="ghost" size="sm" onClick={() => setEditingCreds(false)}>
                Cancel
              </Button>
            )}
          </div>
        </div>
      ) : status?.connected ? (
        <div className="flex items-center gap-4 flex-wrap">
          <p className="text-sm">
            Connected as <span className="font-medium">@{status.username}</span>
          </p>
          <Button variant="outline" size="sm" disabled={busy} onClick={handleDisconnect}>
            Disconnect
          </Button>
          <Button variant="ghost" size="sm" onClick={() => setEditingCreds(true)}>
            Change API key
          </Button>
        </div>
      ) : status?.pending ? (
        <div className="space-y-3 max-w-md">
          <p className="text-sm font-medium">Authenticate</p>
          <p className="text-xs text-muted-foreground">
            Authorise Halo on Last.fm and click <span className="font-medium">Allow</span> —
            Halo finishes connecting automatically. Open the page here, or copy the
            link to open it in another browser.
          </p>
          <div className="flex items-center gap-2 flex-wrap">
            <Button size="sm" onClick={handleOpenInBrowser}>
              Open in browser
            </Button>
            <Button variant="outline" size="sm" onClick={handleCopyLink}>
              {copied ? "Copied!" : "Copy link"}
            </Button>
          </div>
          <Button variant="ghost" size="sm" disabled={busy} onClick={handleCompleteAuth}>
            {busy ? "Connecting…" : "I've authorised — complete setup"}
          </Button>
        </div>
      ) : (
        <div className="space-y-2 max-w-md">
          <p className="text-xs text-muted-foreground">
            Your password never touches this app — you authorise it on Last.fm.
          </p>
          <div className="flex items-center gap-2">
            <Button variant="outline" size="sm" disabled={busy} onClick={handleConnect}>
              {busy ? "Preparing…" : "Connect with browser"}
            </Button>
            <Button variant="ghost" size="sm" onClick={() => setEditingCreds(true)}>
              Change API key
            </Button>
          </div>
        </div>
      )}

      {error && <p className="text-sm text-destructive mt-2">{error}</p>}
    </div>
  );
}

// ── Section: Advanced ──────────────────────────────────────────────────────────

function AdvancedSection({
  scanning,
  onClearCache,
  onResetStats,
  onClearDb,
}: {
  scanning: boolean;
  onClearCache: () => void;
  onResetStats: () => void;
  onClearDb: () => void;
}) {
  return (
    <div>
      <SectionHeader title="Advanced" description="Optional integrations and destructive actions." />

      <LastFmPanel />

      <div className="mt-8 pt-6 border-t border-border">
        <div className="text-sm font-medium mb-1">Destructive actions</div>
        <p className="text-xs text-muted-foreground mb-4">Use with care — these cannot be undone.</p>
        <div className="flex items-center gap-3 flex-wrap">
          <Button variant="outline" size="sm" onClick={onClearCache} disabled={scanning}>
            <Eraser size={15} className="mr-2" />
            Clear cache
          </Button>
          <Button variant="outline" size="sm" onClick={onResetStats} disabled={scanning}>
            <BarChart2 size={15} className="mr-2" />
            Reset play statistics
          </Button>
          <Button variant="outline" size="sm" onClick={onClearDb} disabled={scanning}>
            <Trash2 size={15} className="mr-2" />
            Clear database
          </Button>
        </div>
      </div>
    </div>
  );
}

// ── Main Settings component ────────────────────────────────────────────────────

// Per-tab scroll positions, kept at module scope so they survive leaving and
// returning to Settings. Each category scrolls independently.
const tabScroll = new Map<Category, number>();

export default function Settings() {
  const contentScrollRef = useRef<HTMLDivElement>(null);

  const [searchParams, setSearchParams] = useSearchParams();
  // "services" was merged into "advanced"; map any stale tab refs (old
  // deep-links / remembered tab) so they land on the right section.
  const normalizeTab = (v: string | null | undefined) =>
    v === "services" ? "advanced" : v;
  const tabParam = normalizeTab(searchParams.get("tab"));
  const isCategory = (v: string | null | undefined): v is Category =>
    !!v && CATEGORIES.some((c) => c.id === v);
  const remembered = normalizeTab(getLastTab("settings"));
  const [activeCategory, setActiveCategory] = useState<Category>(
    isCategory(tabParam) ? tabParam : isCategory(remembered) ? remembered : "general",
  );

  // Reflect the active category in the URL (so each tab scrolls independently);
  // when arriving via the sidebar with no ?tab=, restore the remembered tab.
  useEffect(() => {
    if (!isCategory(tabParam)) {
      setSearchParams({ tab: activeCategory }, { replace: true });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Follow ?tab= changes when navigating to Settings while already mounted
  // (e.g. the now-playing EQ shortcut → ?tab=equalizer).
  useEffect(() => {
    if (isCategory(tabParam)) setActiveCategory(tabParam);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tabParam]);

  const selectCategory = (cat: Category) => {
    // Save the outgoing tab's scroll before switching.
    if (contentScrollRef.current) {
      tabScroll.set(activeCategory, contentScrollRef.current.scrollTop);
    }
    setActiveCategory(cat);
    setLastTab("settings", cat);
    setSearchParams({ tab: cat }, { replace: true });
  };

  // Restore each tab's own scroll position before paint (no flash); also keeps
  // the active tab's position current as the user scrolls.
  useLayoutEffect(() => {
    const el = contentScrollRef.current;
    if (el) el.scrollTop = tabScroll.get(activeCategory) ?? 0;
  }, [activeCategory]);

  useEffect(() => {
    const el = contentScrollRef.current;
    if (!el) return;
    const onScroll = () => tabScroll.set(activeCategory, el.scrollTop);
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => el.removeEventListener("scroll", onScroll);
  }, [activeCategory]);

  // General
  const [closeBehavior, setCloseBehaviorState] = useState("minimize");
  const [resumeOnLaunch, setResumeOnLaunchState] = useState(false);

  // Appearance
  const theme = useThemeStore((s) => s.theme);
  const setThemeAction = useThemeStore((s) => s.setTheme);

  // Playback
  const [crossfadeMs, setCrossfadeMsState] = useState(0);

  // Equalizer
  const defaultEq: EqConfig = {
    bypass: false,
    bands: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    stereo: false,
    stereo_width: 1.0,
    dynamic: false,
  };
  const [eq, setEq] = useState<EqConfig>(defaultEq);
  const [userPresets, setUserPresets] = useState<UserPreset[]>([]);
  const [savePresetName, setSavePresetName] = useState("");
  const [savingPreset, setSavingPreset] = useState(false);
  const [currentDevice, setCurrentDevice] = useState<DeviceEqProfile | null>(null);
  const [savingDeviceProfile, setSavingDeviceProfile] = useState(false);

  // Library
  const [folders, setFolders] = useState<Folder[]>([]);
  const [loading, setLoading] = useState(true);
  const [scanning, setScanning] = useState(false);
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [summary, setSummary] = useState<ScanSummary | null>(null);
  const [overrideMetadata, setOverrideMetadata] = useState(false);
  const [autoScanOnAdd, setAutoScanOnAddState] = useState(false);
  const [watchFolders, setWatchFoldersState] = useState(false);
  const [scanSettings, setScanSettingsState] = useState<ScanSettings>({
    delimiters: ",;|:&",
    extensions: "mp3,flac,m4a,aac,ogg,wav,opus,wma,aiff,aif",
  });

  // Services

  // Errors + dialogs
  const [error, setError] = useState<string | null>(null);
  const [confirmClearDb, setConfirmClearDb] = useState(false);
  const [confirmResetStats, setConfirmResetStats] = useState(false);

  const unlistenRef = useRef<UnlistenFn | null>(null);

  // ── Load all settings on mount ──────────────────────────────────────────────

  const loadFolders = useCallback(async () => {
    try {
      setFolders(await getFolders());
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadFolders();
    getScanSettings().then(setScanSettingsState).catch((e) => setError(String(e)));
    getCrossfadeMs().then(setCrossfadeMsState).catch((e) => setError(String(e)));
    getEq().then(setEq).catch((e) => setError(String(e)));
    listUserPresets().then(setUserPresets).catch((e) => setError(String(e)));
    getCurrentDevice().then(setCurrentDevice).catch((e) => setError(String(e)));

    // Generic prefs
    getPref("ui.close_behavior").then((v) => setCloseBehaviorState(v ?? "minimize")).catch(() => {});
    getPref("playback.resume_on_launch").then((v) => setResumeOnLaunchState(v === "true")).catch(() => {});
    getPref("library.auto_scan_on_add").then((v) => setAutoScanOnAddState(v === "true")).catch(() => {});
    getPref("library.watch_folders").then((v) => setWatchFoldersState(v === "true")).catch(() => {});
  }, [loadFolders]);

  useEffect(() => {
    let mounted = true;
    listen<ScanProgress>("scan-progress", (event) => {
      if (!mounted) return;
      setProgress(event.payload);
    }).then((unlisten) => {
      unlistenRef.current = unlisten;
    });
    const unlistenEq = listen<EqConfig>("eq-state-changed", (event) => {
      if (!mounted) return;
      setEq(event.payload);
      getCurrentDevice().then(setCurrentDevice).catch(() => {});
    });
    return () => {
      mounted = false;
      unlistenRef.current?.();
      unlistenEq.then((fn) => fn());
    };
  }, []);

  // ── General handlers ────────────────────────────────────────────────────────

  const handleCloseBehaviorChange = (v: string) => {
    setCloseBehaviorState(v);
    setPref("ui.close_behavior", v).catch((e) => setError(String(e)));
  };

  const handleResumeOnLaunchChange = (v: boolean) => {
    setResumeOnLaunchState(v);
    setPref("playback.resume_on_launch", String(v)).catch((e) => setError(String(e)));
  };

  // ── Playback handlers ───────────────────────────────────────────────────────

  const handleCrossfadeChange = (ms: number) => setCrossfadeMsState(ms);
  const handleCrossfadeCommit = (ms: number) => {
    setCrossfadeMs(ms).catch((e) => setError(String(e)));
  };

  // ── EQ handlers ─────────────────────────────────────────────────────────────

  const handleEqBandChange = (band: number, gain: number) => {
    setEq((prev) => {
      const next = [...prev.bands] as EqConfig["bands"];
      next[band] = gain;
      return { ...prev, bands: next };
    });
  };
  const handleEqBandCommit = (band: number, gain: number) => {
    setEqBand(band, gain).catch((e) => setError(String(e)));
  };
  const handleEqBypassToggle = () => {
    const next = !eq.bypass;
    setEq((prev) => ({ ...prev, bypass: next }));
    setEqBypass(next).catch((e) => setError(String(e)));
  };
  const handleEqPreset = (preset: string) => {
    setEqPreset(preset).then(setEq).catch((e) => setError(String(e)));
  };
  const handleStereoToggle = (enabled: boolean) => {
    setEq((prev) => ({ ...prev, stereo: enabled }));
    setEqStereo(enabled, eq.stereo_width).catch((e) => setError(String(e)));
  };
  const handleStereoWidthChange = (width: number) => {
    setEq((prev) => ({ ...prev, stereo_width: width }));
  };
  const handleStereoWidthCommit = (width: number) => {
    setEqStereo(eq.stereo, width).catch((e) => setError(String(e)));
  };
  const handleDynamicToggle = (enabled: boolean) => {
    setEq((prev) => ({ ...prev, dynamic: enabled }));
    setEqDynamic(enabled).catch((e) => setError(String(e)));
  };
  const handleLoadUserPreset = (id: number) => {
    loadUserPreset(id).then(setEq).catch((e) => setError(String(e)));
  };
  const handleSaveUserPreset = async () => {
    const name = savePresetName.trim();
    if (!name) return;
    setSavingPreset(true);
    try {
      const saved = await saveUserPreset(name);
      setUserPresets((prev) => {
        const idx = prev.findIndex((p) => p.id === saved.id);
        return idx >= 0
          ? prev.map((p) => (p.id === saved.id ? saved : p))
          : [...prev, saved].sort((a, b) => a.name.localeCompare(b.name));
      });
      setSavePresetName("");
    } catch (e) {
      setError(String(e));
    } finally {
      setSavingPreset(false);
    }
  };
  const handleDeleteUserPreset = async (id: number) => {
    try {
      await deleteUserPreset(id);
      setUserPresets((prev) => prev.filter((p) => p.id !== id));
    } catch (e) {
      setError(String(e));
    }
  };
  const handleSaveDeviceProfile = async () => {
    setSavingDeviceProfile(true);
    try {
      const profile = await saveDeviceEqProfile();
      setCurrentDevice(profile);
    } catch (e) {
      setError(String(e));
    } finally {
      setSavingDeviceProfile(false);
    }
  };
  const handleDeleteDeviceProfile = async () => {
    if (!currentDevice) return;
    try {
      await deleteDeviceEqProfile(currentDevice.device_name);
      setCurrentDevice((prev) =>
        prev ? { ...prev, bypass: false, bands: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] } : null
      );
      getCurrentDevice().then(setCurrentDevice).catch(() => {});
    } catch (e) {
      setError(String(e));
    }
  };

  // ── Library handlers ────────────────────────────────────────────────────────

  const handleAddFolder = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (!selected) return;
    try {
      const folder = await addFolder(selected as string);
      setFolders((prev) =>
        prev.some((f) => f.id === folder.id) ? prev : [...prev, folder]
      );
      if (autoScanOnAdd) {
        handleScan();
      }
    } catch (e) {
      setError(String(e));
    }
  };

  const handleRemoveFolder = async (id: number) => {
    try {
      await removeFolder(id);
      setFolders((prev) => prev.filter((f) => f.id !== id));
    } catch (e) {
      setError(String(e));
    }
  };

  const handleScan = async () => {
    setError(null);
    setSummary(null);
    setProgress({
      current: 0, total: 0, current_file: "", inserted: 0,
      updated: 0, skipped: 0, failed: 0, done: false,
    });
    setScanning(true);
    try {
      const result = await scanLibrary(overrideMetadata);
      setSummary(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setScanning(false);
    }
  };

  const handleScanSettingsChange = async (next: ScanSettings) => {
    setScanSettingsState(next);
    try {
      await setScanSettings(next);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleAutoScanOnAddChange = (v: boolean) => {
    setAutoScanOnAddState(v);
    setPref("library.auto_scan_on_add", String(v)).catch((e) => setError(String(e)));
  };

  const handleWatchFoldersChange = (v: boolean) => {
    setWatchFoldersState(v);
    setWatchFolders(v).catch((e) => setError(String(e)));
  };

  // ── Advanced handlers ───────────────────────────────────────────────────────

  const handleClearCache = async () => {
    try {
      await clearCache();
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleClearDatabase = async () => {
    try {
      await clearDatabase();
      setConfirmClearDb(false);
      setSummary(null);
      setProgress(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleResetStats = async () => {
    try {
      await resetAllStats();
      setConfirmResetStats(false);
    } catch (e) {
      setError(String(e));
    }
  };

  // Last.fm is fully self-contained in <LastFmPanel/> (status, creds, auth poll).

  // ── Render ──────────────────────────────────────────────────────────────────

  return (
    <div className="flex h-full">
      {/* Sidebar */}
      <nav className="w-48 shrink-0 border-r border-border p-2 space-y-0.5 overflow-y-auto">
        {CATEGORIES.map((cat) => {
          const Icon = cat.icon;
          const active = activeCategory === cat.id;
          return (
            <button
              key={cat.id}
              onClick={() => selectCategory(cat.id)}
              className={cn(
                "w-full flex items-center gap-2.5 rounded-md px-3 py-2 text-sm transition-colors text-left",
                active
                  ? "bg-muted text-foreground font-medium"
                  : "text-muted-foreground hover:bg-muted/60 hover:text-foreground",
              )}
            >
              <Icon size={15} className="shrink-0" />
              {cat.label}
            </button>
          );
        })}
      </nav>

      {/* Content */}
      <div ref={contentScrollRef} className="flex-1 overflow-y-auto">
        <div className="p-6 max-w-2xl">
          {error && (
            <p className="text-sm text-destructive mb-4">{error}</p>
          )}

          {activeCategory === "general" && (
            <GeneralSection
              closeBehavior={closeBehavior}
              onCloseBehaviorChange={handleCloseBehaviorChange}
              resumeOnLaunch={resumeOnLaunch}
              onResumeOnLaunchChange={handleResumeOnLaunchChange}
            />
          )}

          {activeCategory === "appearance" && (
            <AppearanceSection theme={theme} onThemeChange={setThemeAction} />
          )}

          {activeCategory === "playback" && (
            <PlaybackSection
              crossfadeMs={crossfadeMs}
              onCrossfadeChange={handleCrossfadeChange}
              onCrossfadeCommit={handleCrossfadeCommit}
            />
          )}

          {activeCategory === "equalizer" && (
            <EqualizerSection
              eq={eq}
              userPresets={userPresets}
              currentDevice={currentDevice}
              savingDeviceProfile={savingDeviceProfile}
              savePresetName={savePresetName}
              savingPreset={savingPreset}
              onBypassToggle={handleEqBypassToggle}
              onPreset={handleEqPreset}
              onLoadUserPreset={handleLoadUserPreset}
              onSaveUserPreset={handleSaveUserPreset}
              onDeleteUserPreset={handleDeleteUserPreset}
              onBandChange={handleEqBandChange}
              onBandCommit={handleEqBandCommit}
              onSaveDeviceProfile={handleSaveDeviceProfile}
              onDeleteDeviceProfile={handleDeleteDeviceProfile}
              onSavePresetNameChange={setSavePresetName}
              onStereoToggle={handleStereoToggle}
              onStereoWidthChange={handleStereoWidthChange}
              onStereoWidthCommit={handleStereoWidthCommit}
              onDynamicToggle={handleDynamicToggle}
            />
          )}

          {activeCategory === "library" && (
            <LibrarySection
              folders={folders}
              loading={loading}
              scanning={scanning}
              overrideMetadata={overrideMetadata}
              progress={progress}
              autoScanOnAdd={autoScanOnAdd}
              watchFolders={watchFolders}
              scanSettings={scanSettings}
              onAddFolder={handleAddFolder}
              onRemoveFolder={handleRemoveFolder}
              onScan={handleScan}
              onOverrideMetadataChange={setOverrideMetadata}
              onAutoScanOnAddChange={handleAutoScanOnAddChange}
              onWatchFoldersChange={handleWatchFoldersChange}
              onScanSettingsChange={handleScanSettingsChange}
            />
          )}

          {activeCategory === "advanced" && (
            <AdvancedSection
              scanning={scanning}
              onClearCache={handleClearCache}
              onResetStats={() => setConfirmResetStats(true)}
              onClearDb={() => setConfirmClearDb(true)}
            />
          )}

          {summary && progress?.done && summary.total === 0 && (
            <p className="text-sm text-muted-foreground mt-4">
              No audio files found in the managed folders.
            </p>
          )}
        </div>
      </div>

      {/* Dialogs */}
      <Dialog open={confirmResetStats} onOpenChange={setConfirmResetStats}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Reset play statistics?</DialogTitle>
            <DialogDescription>
              This clears play counts, skip counts, last played timestamps, and star ratings for
              every track. This cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <DialogClose render={<Button variant="outline" size="sm">Cancel</Button>} />
            <Button variant="destructive" size="sm" onClick={handleResetStats}>
              Reset statistics
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={confirmClearDb} onOpenChange={setConfirmClearDb}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Clear database?</DialogTitle>
            <DialogDescription>
              This removes every scanned track, playlist, and queue entry. Managed folders are
              kept. This cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <DialogClose render={<Button variant="outline" size="sm">Cancel</Button>} />
            <Button variant="destructive" size="sm" onClick={handleClearDatabase}>
              Clear database
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
