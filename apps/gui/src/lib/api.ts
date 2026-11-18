import { invoke } from "@tauri-apps/api/core";
import type { DeviceView } from "./bindings/DeviceView";
import type { Holder } from "./bindings/Holder";
import type { Event } from "./bindings/Event";

export const listDevices = () => invoke<DeviceView[]>("list_devices");
export const usbHint = () => invoke<string | null>("usb_hint");
export const deviceHolders = (path: string) => invoke<Holder[]>("device_holders", { path });
export const historyEvents = (key: string) => invoke<Event[]>("history_events", { key });
