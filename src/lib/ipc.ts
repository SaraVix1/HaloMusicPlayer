import { invoke } from "@tauri-apps/api/core";

export interface Folder {
  id: number;
  path: string;
  added_at: string;
}

export const getFolders = () => invoke<Folder[]>("get_folders");
export const addFolder = (path: string) => invoke<Folder>("add_folder", { path });
export const removeFolder = (id: number) => invoke<void>("remove_folder", { id });

export interface ScanProgress {
  current: number;
  total: number;
  current_file: string;
  inserted: number;
  updated: number;
  skipped: number;
  failed: number;
  done: boolean;
}

export interface ScanSummary {
  total: number;
  inserted: number;
  updated: number;
  skipped: number;
  failed: number;
}

export interface ScanSettings {
  delimiters: string;
  extensions: string;
}

export const scanLibrary = (overrideMetadata: boolean) =>
  invoke<ScanSummary>("scan_library", { overrideMetadata });
export const clearCache = () => invoke<void>("clear_cache");
export const clearDatabase = () => invoke<void>("clear_database");
export const getScanSettings = () => invoke<ScanSettings>("get_scan_settings");
export const setScanSettings = (settings: ScanSettings) =>
  invoke<void>("set_scan_settings", { settings });

export interface Track {
  id: number;
  title: string | null;
  album_name: string | null;
  artists: string[];
  genres: string[];
  duration_ms: number | null;
  track_number: number | null;
  disc_number: number | null;
  year: number | null;
  album_art_path: string | null;
  file_path: string;
  folder_id: number | null;
  scanned_at: string;
  rating: number;
  play_count: number;
  skip_count: number;
  last_played_at: number | null;
}

export interface TracksQuery {
  album?: string;
  artist?: string;
  album_artist?: string;
  composer?: string;
  genre?: string;
  year?: number;
  folder_id?: number;
  sort?: string;
  direction?: "asc" | "desc";
}

export interface Album {
  name: string;
  album_artists: string[];
  track_count: number;
  year: number | null;
  album_art_path: string | null;
}

export interface Artist {
  id: number;
  name: string;
  track_count: number;
  album_art_path: string | null;
}

export interface Genre {
  id: number;
  name: string;
  track_count: number;
}

export interface FolderTrack {
  id: number;
  folder_id: number;
  folder_path: string;
  file_path: string;
  title: string | null;
  duration_ms: number | null;
  album_art_path: string | null;
}

export const getTracks = (query: TracksQuery = {}) =>
  invoke<Track[]>("get_tracks", { query });
export const getAlbums = () => invoke<Album[]>("get_albums");
export const getArtists = () => invoke<Artist[]>("get_artists");
export const getGenres = () => invoke<Genre[]>("get_genres");
export const getFolderTracks = () => invoke<FolderTrack[]>("get_folder_tracks");

export interface AlbumArtist {
  id: number;
  name: string;
  track_count: number;
  album_count: number;
  album_art_path: string | null;
}

export interface Composer {
  id: number;
  name: string;
  track_count: number;
}

export interface YearStat {
  year: number;
  track_count: number;
}

export const getRecentlyPlayed = () => invoke<Track[]>("get_recently_played");
export const getMostPlayed = () => invoke<Track[]>("get_most_played");

export const getAlbumArtists = () => invoke<AlbumArtist[]>("get_album_artists");
export const getComposers = () => invoke<Composer[]>("get_composers");
export const getYears = () => invoke<YearStat[]>("get_years");

export type PlaybackStatus = "stopped" | "playing" | "paused";
export type RepeatMode = "off" | "all" | "one";

export interface CurrentTrack {
  track_id: number;
  title: string | null;
  album_name: string | null;
  artists: string[];
  composers: string[];
  album_art_path: string | null;
  file_path: string;
  rating: number;
}

export interface SleepTimerInfo {
  active: boolean;
  end_of_song: boolean;
  remaining_secs: number | null;
  fade: boolean;
}

export interface PlayerState {
  status: PlaybackStatus;
  track_id: number | null;
  position_ms: number;
  duration_ms: number | null;
  volume: number;
  current_index: number | null;
  queue_length: number;
  shuffle: boolean;
  repeat: RepeatMode;
  crossfade_ms: number;
  current_track: CurrentTrack | null;
  sleep_timer: SleepTimerInfo;
}

export interface QueueTrack {
  queue_id: number;
  position: number;
  track_id: number;
  title: string | null;
  album_name: string | null;
  artists: string[];
  duration_ms: number | null;
  album_art_path: string | null;
  file_path: string;
}

export const getPlayerState = () => invoke<PlayerState>("get_player_state");
export const playQueueIndex = (index: number) =>
  invoke<void>("play_queue_index", { index });
export const togglePlayPause = () => invoke<void>("toggle_play_pause");
export const stopPlayback = () => invoke<void>("stop_playback");
export const seekTo = (positionMs: number) =>
  invoke<void>("seek_to", { positionMs });
export const getWaveform = (trackId: number) =>
  invoke<number[]>("get_waveform", { trackId });
export const setVolume = (volume: number) =>
  invoke<void>("set_volume", { volume });
export const nextTrack = () => invoke<void>("next_track");
export const previousTrack = () => invoke<void>("previous_track");
export const setShuffle = (enabled: boolean) =>
  invoke<void>("set_shuffle", { enabled });
export const setRepeat = (mode: RepeatMode) =>
  invoke<void>("set_repeat", { mode });
export const getQueue = () => invoke<QueueTrack[]>("get_queue");
export const setQueueAndPlay = (trackIds: number[], startIndex: number) =>
  invoke<void>("set_queue_and_play", { trackIds, startIndex });
export const addToQueue = (trackId: number) =>
  invoke<void>("add_to_queue", { trackId });
export const removeFromQueue = (queueId: number) =>
  invoke<void>("remove_from_queue", { queueId });
export const clearQueue = () => invoke<void>("clear_queue");
export const getCrossfadeMs = () => invoke<number>("get_crossfade_ms");
export const setCrossfadeMs = (ms: number) =>
  invoke<void>("set_crossfade_ms", { ms });

export interface Playlist {
  id: number;
  name: string;
  track_count: number;
  created_at: string;
  updated_at: string;
}

export interface PlaylistTrack {
  playlist_track_id: number;
  position: number;
  track_id: number;
  title: string | null;
  album_name: string | null;
  artists: string[];
  duration_ms: number | null;
  album_art_path: string | null;
  file_path: string;
}

export const getPlaylists = () => invoke<Playlist[]>("get_playlists");
export const getPlaylist = (id: number) =>
  invoke<[Playlist, PlaylistTrack[]]>("get_playlist", { id });
export const createPlaylist = (name: string) =>
  invoke<Playlist>("create_playlist", { name });
export const renamePlaylist = (id: number, name: string) =>
  invoke<void>("rename_playlist", { id, name });
export const deletePlaylist = (id: number) =>
  invoke<void>("delete_playlist", { id });
export const addToPlaylist = (playlistId: number, trackIds: number[]) =>
  invoke<void>("add_to_playlist", { playlistId, trackIds });
export const removeFromPlaylist = (playlistTrackId: number) =>
  invoke<void>("remove_from_playlist", { playlistTrackId });
export const reorderPlaylistTrack = (
  playlistTrackId: number,
  newPosition: number,
) =>
  invoke<void>("reorder_playlist_track", {
    playlistTrackId,
    newPosition,
  });

export interface SearchTrackHit {
  track_id: number;
  title: string;
  album_name: string | null;
  artists: string[];
  album_art_path: string | null;
  duration_ms: number | null;
}

export interface SearchAlbumHit {
  name: string;
  album_art_path: string | null;
  track_count: number;
}

export interface SearchArtistHit {
  id: number;
  name: string;
  track_count: number;
}

export interface SearchPlaylistHit {
  id: number;
  name: string;
  track_count: number;
}

export interface SearchResults {
  tracks: SearchTrackHit[];
  albums: SearchAlbumHit[];
  artists: SearchArtistHit[];
  playlists: SearchPlaylistHit[];
}

export const searchLibrary = (query: string) =>
  invoke<SearchResults>("search_library", { query });

export type Theme = "light" | "dark" | "system";

export const getTheme = () => invoke<Theme>("get_theme");
export const setTheme = (theme: Theme) =>
  invoke<void>("set_theme", { theme });

export const openMiniPlayer = () => invoke<void>("open_mini_player");
export const restoreMainWindow = () => invoke<void>("restore_main_window");
export const saveMiniPosition = (x: number, y: number) =>
  invoke<void>("save_mini_position", { x, y });

export const setRating = (trackId: number, rating: number) =>
  invoke<void>("set_rating", { trackId, rating });
export const resetAllStats = () => invoke<void>("reset_all_stats");


export interface FullTrackMetadata {
  id: number;
  file_path: string;
  album_art_path: string | null;
  title: string | null;
  album_name: string | null;
  artists: string[];
  album_artists: string[];
  composers: string[];
  genres: string[];
  year: number | null;
  track_number: number | null;
  track_total: number | null;
  disc_number: number | null;
  disc_total: number | null;
  comment: string | null;
  publisher: string | null;
  copyright: string | null;
  language: string | null;
}

export interface MetadataEdit {
  title: string | null;
  album_name: string | null;
  artists: string[];
  album_artists: string[];
  composers: string[];
  genres: string[];
  year: number | null;
  track_number: number | null;
  track_total: number | null;
  disc_number: number | null;
  disc_total: number | null;
  comment: string | null;
  publisher: string | null;
  copyright: string | null;
  language: string | null;
  new_art_path: string | null;
}

export interface CoverSuggestion {
  thumbnail_url: string;
  full_url: string;
  title: string;
  date: string | null;
}

export const getTrackFullMetadata = (trackId: number) =>
  invoke<FullTrackMetadata>("get_track_full_metadata", { trackId });
export const saveTrackMetadata = (trackId: number, edit: MetadataEdit) =>
  invoke<void>("save_track_metadata", { trackId, edit });
export const extractTrackArt = (trackId: number) =>
  invoke<string>("extract_track_art", { trackId });
export const fetchArtFromUrl = (url: string) =>
  invoke<string>("fetch_art_from_url", { url });
export const searchCoverArt = (artist: string, album: string) =>
  invoke<CoverSuggestion[]>("search_cover_art", { artist, album });
export const processArt = (
  sourcePath: string,
  cropX: number,
  cropY: number,
  cropW: number,
  cropH: number,
) => invoke<string>("process_art", { sourcePath, cropX, cropY, cropW, cropH });

export const setSleepTimer = (minutes: number, fade: boolean) =>
  invoke<void>("set_sleep_timer", { minutes, fade });
export const setSleepTimerEndOfSong = (fade: boolean) =>
  invoke<void>("set_sleep_timer_end_of_song", { fade });
export const cancelSleepTimer = () => invoke<void>("cancel_sleep_timer");
export const getSleepTimer = () => invoke<SleepTimerInfo>("get_sleep_timer");

export interface LyricsLine {
  time_ms: number;
  text: string;
}

export interface LyricsResult {
  source: "database" | "lrc_file" | "lrclib" | "jiosaavn" | "lyricsovh" | "none" | "not_found";
  synced: boolean;
  lines: LyricsLine[];
}

export interface LyricsCandidate {
  provider: string;
  label: string;    // track · artist from the source; may be empty
  synced: boolean;
  snippet: string;
  lyrics: string;
}

export const getLyrics = (trackId: number) =>
  invoke<LyricsResult>("get_lyrics", { trackId });
export const saveLyrics = (trackId: number, lyrics: string) =>
  invoke<void>("save_lyrics", { trackId, lyrics });
export const fetchLyricsOnline = (trackId: number) =>
  invoke<LyricsResult>("fetch_lyrics_online", { trackId });
export const searchLyricsProviders = (
  trackId: number,
  title: string,
  artist: string,
  album: string,
) => invoke<LyricsCandidate[]>("search_lyrics_providers", { trackId, title, artist, album });

export const EQ_BAND_FREQS = [32, 64, 125, 250, 500, 1000, 2000, 4000, 8000, 16000] as const;
export const EQ_BAND_LABELS = ["32", "64", "125", "250", "500", "1k", "2k", "4k", "8k", "16k"] as const;
export const EQ_PRESETS = ["Flat", "Rock", "Pop", "Jazz", "Classical", "Electronic", "Bass Boost", "Treble Boost", "Vocal", "Acoustic"] as const;
export type EqPreset = typeof EQ_PRESETS[number];

// Mirror of the gain tables in src-tauri/src/commands/eq.rs (preset_gains).
// Kept in sync so the UI can highlight which preset is currently active.
export const EQ_PRESET_GAINS: Record<EqPreset, number[]> = {
  "Flat":         [ 0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0],
  "Rock":         [ 4.0,  3.0, -1.0, -1.0,  0.0,  1.0,  2.0,  3.0,  3.0,  3.0],
  "Pop":          [-1.5, -1.0,  0.0,  2.0,  4.0,  4.0,  2.0,  0.0, -1.0, -1.5],
  "Jazz":         [ 3.0,  2.0,  1.0,  2.0, -2.0, -2.0,  0.0,  1.0,  2.0,  3.0],
  "Classical":    [ 4.0,  3.0, -1.0, -1.0, -1.0,  0.0,  0.0,  1.0,  3.0,  4.0],
  "Electronic":   [ 4.0,  3.5,  1.0,  0.0, -1.0,  1.0,  0.0,  1.0,  3.0,  4.0],
  "Bass Boost":   [ 6.0,  5.0,  4.0,  2.0,  1.0,  0.0,  0.0,  0.0,  0.0,  0.0],
  "Treble Boost": [ 0.0,  0.0,  0.0,  0.0,  0.0,  1.0,  2.0,  4.0,  5.0,  6.0],
  "Vocal":        [-2.0, -2.0, -1.0,  2.0,  4.0,  4.0,  3.0,  2.0, -1.0, -2.0],
  "Acoustic":     [ 4.0,  3.0,  2.0,  1.0,  2.0,  3.0,  4.0,  3.0,  2.0,  1.0],
};

/** Returns the built-in preset whose gains match `bands`, or null for a custom curve. */
export function matchEqPreset(bands: readonly number[]): EqPreset | null {
  for (const name of EQ_PRESETS) {
    const g = EQ_PRESET_GAINS[name];
    if (g.every((v, i) => Math.abs(v - bands[i]) < 0.05)) return name;
  }
  return null;
}

export const STEREO_WIDTH_MAX = 3.0;

export interface EqConfig {
  bypass: boolean;
  bands: [number, number, number, number, number, number, number, number, number, number];
  stereo: boolean;
  stereo_width: number;
  dynamic: boolean;
}

// Smart playlists

export interface SmartPlaylist {
  id: number;
  name: string;
  match_mode: "all" | "any";
  sort_field: string;
  sort_direction: "asc" | "desc";
  limit_count: number | null;
  created_at: number;
  updated_at: number;
}

export interface SmartPlaylistRule {
  id: number;
  playlist_id: number;
  field: string;
  operator: string;
  value: string;
  position: number;
}

export interface SmartTrack {
  id: number;
  title: string | null;
  album_name: string | null;
  artists: string[];
  duration_ms: number | null;
  album_art_path: string | null;
  file_path: string;
  rating: number;
  play_count: number;
}

export const getSmartPlaylists = () =>
  invoke<SmartPlaylist[]>("get_smart_playlists");
export const getSmartPlaylist = (id: number) =>
  invoke<[SmartPlaylist, SmartPlaylistRule[]]>("get_smart_playlist", { id });
export const createSmartPlaylist = (name: string) =>
  invoke<SmartPlaylist>("create_smart_playlist", { name });
export const updateSmartPlaylist = (
  id: number,
  name: string,
  matchMode: SmartPlaylist["match_mode"],
  sortField: string,
  sortDirection: SmartPlaylist["sort_direction"],
  limitCount: number | null,
) =>
  invoke<SmartPlaylist>("update_smart_playlist", {
    id, name, matchMode, sortField, sortDirection, limitCount,
  });
export const deleteSmartPlaylist = (id: number) =>
  invoke<void>("delete_smart_playlist", { id });
export const setSmartPlaylistRules = (
  playlistId: number,
  rules: SmartPlaylistRule[],
) => invoke<void>("set_smart_playlist_rules", { playlistId, rules });
export const getSmartPlaylistTracks = (id: number) =>
  invoke<SmartTrack[]>("get_smart_playlist_tracks", { id });

export interface LastFmStatus {
  connected: boolean;
  username: string | null;
  pending: boolean;
  configured: boolean;
}

export const lastfmGetStatus = () => invoke<LastFmStatus>("lastfm_get_status");
export const lastfmSetCredentials = (apiKey: string, apiSecret: string) =>
  invoke<void>("lastfm_set_credentials", { apiKey, apiSecret });
export const lastfmStartAuth = () => invoke<string>("lastfm_start_auth");
export const lastfmCompleteAuth = () => invoke<string>("lastfm_complete_auth");
export const lastfmLogout = () => invoke<void>("lastfm_logout");
export const lastfmLove = (trackId: number, love: boolean) =>
  invoke<void>("lastfm_love", { trackId, love });
export const lastfmIsLoved = (trackId: number) =>
  invoke<boolean>("lastfm_is_loved", { trackId });

export const getPref = (key: string) => invoke<string | null>("get_pref", { key });
export const setPref = (key: string, value: string) => invoke<void>("set_pref", { key, value });
export const setWatchFolders = (enabled: boolean) =>
  invoke<void>("set_watch_folders", { enabled });

export const getEq = () => invoke<EqConfig>("get_eq");
export const setEqBand = (band: number, gainDb: number) =>
  invoke<void>("set_eq_band", { band, gainDb });
export const setEqBypass = (bypass: boolean) =>
  invoke<void>("set_eq_bypass", { bypass });
export const setEqPreset = (preset: string) =>
  invoke<EqConfig>("set_eq_preset", { preset });
export const setEqStereo = (enabled: boolean, width: number) =>
  invoke<void>("set_eq_stereo", { enabled, width });
export const setEqDynamic = (enabled: boolean) =>
  invoke<void>("set_eq_dynamic", { enabled });

export interface UserPreset {
  id: number;
  name: string;
  bands: [number, number, number, number, number, number, number, number, number, number];
}

export const listUserPresets = () => invoke<UserPreset[]>("list_user_presets");
export const saveUserPreset = (name: string) =>
  invoke<UserPreset>("save_user_preset", { name });
export const loadUserPreset = (id: number) =>
  invoke<EqConfig>("load_user_preset", { id });
export const deleteUserPreset = (id: number) =>
  invoke<void>("delete_user_preset", { id });

export interface DeviceEqProfile {
  device_name: string;
  bypass: boolean;
  bands: [number, number, number, number, number, number, number, number, number, number];
}

export const getCurrentDevice = () => invoke<DeviceEqProfile>("get_current_device");
export const saveDeviceEqProfile = () => invoke<DeviceEqProfile>("save_device_eq_profile");
export const deleteDeviceEqProfile = (deviceName: string) =>
  invoke<void>("delete_device_eq_profile", { deviceName });
export const listDeviceEqProfiles = () => invoke<DeviceEqProfile[]>("list_device_eq_profiles");
