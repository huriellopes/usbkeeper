import { create } from "zustand";
import type { DeviceView } from "./bindings/DeviceView";
import type { OpProgress } from "./bindings/OpProgress";
import type { OpFinished } from "./bindings/OpFinished";

export type RunningOp = {
  id: number;
  kind: string;
  startedAt: number;
  phaseStartedAt: number;
  progress: OpProgress | null;
  cancelling: boolean;
};

type AppState = {
  devices: DeviceView[];
  selected: string | null; // device.path
  error: string | null;
  ops: Record<string, RunningOp>; // device → operação em andamento
  finished: Record<string, Record<string, OpFinished>>; // device → kind → último resultado
  help: { open: boolean; topic: string };
  /** Aviso no topo da janela (ex.: "pode tirar"); sobrevive ao disco sumir da lista. */
  notice: { text: string; tone: "ok" | "warn" } | null;
  dismissNotice: () => void;
  openHelp: (topic?: string) => void;
  closeHelp: () => void;
  setDevices: (d: DeviceView[]) => void;
  setError: (e: string | null) => void;
  select: (path: string) => void;
  opStarted: (device: string, id: number, kind: string) => void;
  opProgress: (device: string, id: number, progress: OpProgress) => void;
  opCancelling: (device: string) => void;
  opFinished: (f: OpFinished) => void;
};

let lastSnapshot = "";

/** Texto do aviso depois de remover: o nome vem da lista atual (o disco
 *  some dela logo em seguida). */
function ejectNotice(devices: DeviceView[], f: OpFinished): AppState["notice"] {
  const d = devices.find((v) => v.device.path === f.device)?.device;
  const name = d ? `${d.model ?? d.name} (${d.name})` : f.device;
  const r = (f.result ?? {}) as { powered_off?: boolean };
  return r.powered_off
    ? { text: `✔ Pode tirar o ${name}.`, tone: "ok" }
    : { text: `✔ ${name} desmontado. Pode tirar quando o LED parar de piscar.`, tone: "warn" };
}

export const useApp = create<AppState>((set, get) => ({
  devices: [],
  selected: null,
  error: null,
  ops: {},
  finished: {},
  help: { open: false, topic: "comecar" },
  notice: null,
  dismissNotice: () => set({ notice: null }),
  openHelp: (topic) => set((s) => ({ help: { open: true, topic: topic ?? s.help.topic } })),
  closeHelp: () => set((s) => ({ help: { ...s.help, open: false } })),
  setDevices: (devices) => {
    // Resposta igual à anterior: não troca o estado (evita redesenhar a cada 2 s).
    const snapshot = JSON.stringify(devices);
    if (snapshot === lastSnapshot && get().error === null) return;
    lastSnapshot = snapshot;
    const sel = get().selected;
    const stillThere = devices.some((d) => d.device.path === sel);
    const firstUsable = devices.find((d) => !d.is_system)?.device.path ?? devices[0]?.device.path ?? null;
    set({ devices, error: null, selected: stillThere ? sel : firstUsable });
  },
  setError: (error) => set({ error }),
  select: (selected) => set({ selected }),
  opStarted: (device, id, kind) =>
    set((s) => ({
      ops: {
        ...s.ops,
        [device]: { id, kind, startedAt: Date.now(), phaseStartedAt: Date.now(), progress: null, cancelling: false },
      },
    })),
  opProgress: (device, id, progress) =>
    set((s) => {
      const op = s.ops[device];
      if (!op || op.id !== id) return s;
      const phaseStartedAt = op.progress?.phase === progress.phase ? op.phaseStartedAt : Date.now();
      return { ops: { ...s.ops, [device]: { ...op, progress, phaseStartedAt } } };
    }),
  opCancelling: (device) =>
    set((s) => (s.ops[device] ? { ops: { ...s.ops, [device]: { ...s.ops[device], cancelling: true } } } : s)),
  opFinished: (f) =>
    set((s) => {
      const { [f.device]: _done, ...ops } = s.ops;
      const notice = f.kind === "eject" && f.ok ? ejectNotice(s.devices, f) : s.notice;
      return {
        ops,
        notice,
        finished: { ...s.finished, [f.device]: { ...(s.finished[f.device] ?? {}), [f.kind]: f } },
      };
    }),
}));
