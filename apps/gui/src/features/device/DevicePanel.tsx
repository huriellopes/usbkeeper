import { useState } from "react";
import type { DeviceView } from "../../lib/bindings/DeviceView";
import { humanBytes } from "../../lib/format";
import { Overview } from "./Overview";

export type TabId = "overview" | "check" | "health" | "bench" | "format" | "history";
export const TABS: { id: TabId; label: string; destructive: boolean }[] = [
  { id: "overview", label: "Visão geral", destructive: false },
];

export function DevicePanel({ view }: { view: DeviceView }) {
  const [tab, setTab] = useState<TabId>("overview");
  const d = view.device;
  return (
    <div className="flex h-full flex-col">
      <header className="border-b px-6 py-4" style={{ borderColor: "var(--line)" }}>
        <h1 className="text-xl font-semibold">{d.model ?? d.name}</h1>
        <p className="text-sm" style={{ color: "var(--muted)" }}>
          {d.path} · {humanBytes(d.size_bytes)}
        </p>
        <nav className="mt-3 flex gap-1" role="tablist">
          {TABS.map((t) => (
            <button
              key={t.id}
              role="tab"
              aria-selected={tab === t.id}
              disabled={t.destructive && view.is_system}
              onClick={() => setTab(t.id)}
              className="rounded-md px-3 py-1 text-sm disabled:opacity-40"
              style={{ background: tab === t.id ? "var(--line)" : "transparent" }}
            >
              {t.label}
            </button>
          ))}
        </nav>
      </header>
      <div className="flex-1 overflow-auto p-6">{tab === "overview" && <Overview view={view} />}</div>
    </div>
  );
}
