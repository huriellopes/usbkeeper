import { useEffect, useState } from "react";
import { guideTopics, listDevices, onOpFinished, onOpProgress, platform, usbHint } from "./lib/api";
import type { GuideTopic } from "./lib/bindings/GuideTopic";
import { useApp } from "./lib/store";
import { Sidebar } from "./features/sidebar/Sidebar";
import { DevicePanel } from "./features/device/DevicePanel";
import { HelpPanel } from "./features/help/HelpPanel";
import { Notice } from "./features/remove/Notice";
import { ThemeSwitch } from "./features/theme/ThemeSwitch";

const POLL_MS = 2000;

export default function App() {
  const { devices, selected, error, setDevices, setError, select, opProgress, opFinished, help, openHelp, closeHelp } =
    useApp();
  const [hint, setHint] = useState<string | null>(null);
  const [topics, setTopics] = useState<GuideTopic[]>([]);
  useEffect(() => {
    guideTopics().then(setTopics).catch(() => setTopics([]));
  }, []);
  useEffect(() => {
    const unlisten = [onOpProgress((e) => opProgress(e.device, e.id, e.progress)), onOpFinished(opFinished)];
    return () => {
      unlisten.forEach((p) => p.then((f) => f()));
    };
  }, [opProgress, opFinished]);
  const setPlatform = useApp((s) => s.setPlatform);
  useEffect(() => {
    platform().then(setPlatform).catch(() => {});
  }, [setPlatform]);
  useEffect(() => {
    usbHint().then(setHint).catch(() => setHint(null));
  }, []);
  useEffect(() => {
    let timer: number | undefined;
    const tick = () => {
      // Só consulta com a janela visível (orçamento de CPU).
      if (document.visibilityState === "visible") {
        listDevices().then(setDevices).catch((e) => setError(String(e)));
      }
      timer = window.setTimeout(tick, POLL_MS);
    };
    tick();
    return () => window.clearTimeout(timer);
  }, [setDevices, setError]);
  const current = devices.find((d) => d.device.path === selected);
  return (
    <div className="grid h-full" style={{ gridTemplateColumns: "300px 1fr" }}>
      <aside className="flex flex-col overflow-hidden border-r" style={{ borderColor: "var(--line)" }}>
        <div className="flex-1 overflow-auto">
          <Sidebar
            devices={devices}
            selected={selected}
            onSelect={(path) => {
              closeHelp();
              select(path);
            }}
            hint={hint}
          />
          {error && <p className="p-4 text-sm" style={{ color: "var(--bad)" }}>{error}</p>}
        </div>
        <div className="flex flex-col gap-2 border-t p-2" style={{ borderColor: "var(--line)" }}>
          <ThemeSwitch />
          <button
            type="button"
            onClick={() => (help.open ? closeHelp() : openHelp())}
            aria-pressed={help.open}
            className="w-full rounded-lg px-3 py-2 text-left text-sm font-medium"
            style={{ background: help.open ? "var(--panel)" : "transparent", border: "1px solid var(--line)" }}
          >
            ? Ajuda e guia
          </button>
        </div>
      </aside>
      <main className="flex flex-col overflow-hidden" style={{ background: "var(--panel)" }}>
        <Notice />
        <div className="min-h-0 flex-1">
          {help.open ? <HelpPanel topics={topics} /> : current ? <DevicePanel key={current.device.path} view={current} /> : null}
        </div>
      </main>
    </div>
  );
}
