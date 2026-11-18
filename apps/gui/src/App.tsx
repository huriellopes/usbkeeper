import { useEffect, useState } from "react";
import { listDevices, usbHint } from "./lib/api";
import { useApp } from "./lib/store";
import { Sidebar } from "./features/sidebar/Sidebar";
import { DevicePanel } from "./features/device/DevicePanel";

const POLL_MS = 2000;

export default function App() {
  const { devices, selected, error, setDevices, setError, select } = useApp();
  const [hint, setHint] = useState<string | null>(null);
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
      <aside className="overflow-auto border-r" style={{ borderColor: "var(--line)" }}>
        <Sidebar devices={devices} selected={selected} onSelect={select} hint={hint} />
        {error && <p className="p-4 text-sm" style={{ color: "var(--bad)" }}>{error}</p>}
      </aside>
      <main className="overflow-hidden" style={{ background: "var(--panel)" }}>
        {current ? <DevicePanel key={current.device.path} view={current} /> : null}
      </main>
    </div>
  );
}
