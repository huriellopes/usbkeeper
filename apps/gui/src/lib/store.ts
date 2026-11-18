import { create } from "zustand";
import type { DeviceView } from "./bindings/DeviceView";

type AppState = {
  devices: DeviceView[];
  selected: string | null; // device.path
  error: string | null;
  setDevices: (d: DeviceView[]) => void;
  setError: (e: string | null) => void;
  select: (path: string) => void;
};

export const useApp = create<AppState>((set, get) => ({
  devices: [],
  selected: null,
  error: null,
  setDevices: (devices) => {
    const sel = get().selected;
    const stillThere = devices.some((d) => d.device.path === sel);
    const firstUsable = devices.find((d) => !d.is_system)?.device.path ?? devices[0]?.device.path ?? null;
    set({ devices, error: null, selected: stillThere ? sel : firstUsable });
  },
  setError: (error) => set({ error }),
  select: (selected) => set({ selected }),
}));
