import { invoke } from "@tauri-apps/api/core";
import type { DeviceView } from "./bindings/DeviceView";
import type { Holder } from "./bindings/Holder";
import type { Event } from "./bindings/Event";
import { listen } from "@tauri-apps/api/event";
import type { OpRequest } from "./bindings/OpRequest";
import type { OpEvent } from "./bindings/OpEvent";
import type { OpFinished } from "./bindings/OpFinished";
import type { GuideTopic } from "./bindings/GuideTopic";

export const listDevices = () => invoke<DeviceView[]>("list_devices");
export const usbHint = () => invoke<string | null>("usb_hint");
export const deviceHolders = (path: string) => invoke<Holder[]>("device_holders", { path });
export const historyEvents = (key: string) => invoke<Event[]>("history_events", { key });

export const startOperation = (request: OpRequest, token: string) =>
  invoke<number>("start_operation", { request, token });
export const cancelOperation = (device: string) => invoke<void>("cancel_operation", { device });
export const onOpProgress = (cb: (e: OpEvent) => void) => listen<OpEvent>("op-progress", (e) => cb(e.payload));
export const onOpFinished = (cb: (e: OpFinished) => void) => listen<OpFinished>("op-finished", (e) => cb(e.payload));
export const saveReport = (key: string, path: string) => invoke<void>("save_report", { key, path });
export const guideTopics = () => invoke<GuideTopic[]>("guide_topics");
