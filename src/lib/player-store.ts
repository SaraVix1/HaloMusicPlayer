import { create } from "zustand";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  getPlayerState,
  type PlayerState,
  type RepeatMode,
} from "@/lib/ipc";

interface PlayerStore extends PlayerState {
  initialized: boolean;
  init: () => Promise<void>;
}

const initial: PlayerState = {
  status: "stopped",
  track_id: null,
  position_ms: 0,
  duration_ms: null,
  volume: 0.75,
  current_index: null,
  queue_length: 0,
  shuffle: false,
  repeat: "off" as RepeatMode,
  crossfade_ms: 0,
  current_track: null,
  sleep_timer: { active: false, end_of_song: false, remaining_secs: null, fade: false },
};

let unlisten: UnlistenFn | null = null;

export const usePlayerStore = create<PlayerStore>((set) => ({
  ...initial,
  initialized: false,
  init: async () => {
    if (unlisten) return;
    try {
      const state = await getPlayerState();
      set({ ...state, initialized: true });
    } catch {
      set({ initialized: true });
    }
    unlisten = await listen<PlayerState>("player-state", (event) => {
      set(event.payload);
    });
  },
}));
