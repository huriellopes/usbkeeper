import { useEffect, useState } from "react";
import type { DeviceView } from "../../lib/bindings/DeviceView";
import { cancelOperation } from "../../lib/api";
import { humanBytes } from "../../lib/format";
import { useApp } from "../../lib/store";
import { BenchTab } from "../bench/BenchTab";
import { FakeCheckTab } from "../check/FakeCheckTab";
import { FormatTab } from "../format/FormatTab";
import { HealthTab } from "../health/HealthTab";
import { HistoryTab } from "../history/HistoryTab";
import { ConfirmDialog } from "../ops/ConfirmDialog";
import { OperationBar } from "../ops/OperationBar";
import { useOperation } from "../ops/useOperation";
import { Overview } from "./Overview";
import { InUseCard } from "../remove/InUseCard";
import { RemoveButton } from "../remove/RemoveButton";

export type TabId = "overview" | "check" | "health" | "bench" | "format" | "history";
export const TABS: { id: TabId; label: string; destructive: boolean }[] = [
  { id: "overview", label: "Visão geral", destructive: false },
  { id: "check", label: "Verificar", destructive: true },
  { id: "health", label: "Saúde", destructive: false },
  { id: "bench", label: "Velocidade", destructive: false },
  { id: "format", label: "Formatar", destructive: true },
  { id: "history", label: "Histórico", destructive: false },
];

export function DevicePanel({ view }: { view: DeviceView }) {
  const [tab, setTab] = useState<TabId>("overview");
  const d = view.device;
  const op = useApp((s) => s.ops[d.path]);
  const opCancelling = useApp((s) => s.opCancelling);
  const [formatLimit, setFormatLimit] = useState<number | undefined>(view.suggested_max_bytes ?? undefined);
  const formatOp = useOperation(view);
  const ejectOp = useOperation(view);
  const ejected = ejectOp.last("eject");

  // Re-renderiza 1x/s enquanto há operação, para o ETA andar.
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (!op) return;
    const t = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(t);
  }, [op]);

  return (
    <div className="flex h-full flex-col">
      <header className="border-b px-6 py-4" style={{ borderColor: "var(--line)" }}>
        <div className="flex items-start justify-between gap-4">
          <div>
            <h1 className="text-xl font-semibold">{d.model ?? d.name}</h1>
            <p className="text-sm" style={{ color: "var(--muted)" }}>
              {d.path} · {humanBytes(d.size_bytes)}
            </p>
          </div>
          {!view.is_system && d.transport === "usb" && (
            <div className="flex flex-col items-end gap-1">
              <RemoveButton view={view} />
              {ejectOp.error && <span className="text-sm" style={{ color: "var(--bad)" }}>{ejectOp.error}</span>}
            </div>
          )}
        </div>
        <nav className="mt-3 flex flex-wrap gap-1" role="tablist">
          {TABS.map((t) => (
            <button
              key={t.id}
              role="tab"
              aria-selected={tab === t.id}
              disabled={t.destructive && view.is_system}
              title={t.destructive && view.is_system ? "Indisponível no disco do sistema" : undefined}
              onClick={() => setTab(t.id)}
              className="rounded-md px-3 py-1 text-sm disabled:opacity-40"
              style={{ background: tab === t.id ? "var(--line)" : "transparent" }}
            >
              {t.label}
            </button>
          ))}
        </nav>
      </header>
      {ejected && !ejected.ok && !op && (
        <div className="px-6 pt-4">
          <InUseCard
            error={ejected.error ?? "não foi possível remover"}
            holders={ejected.holders}
            onRetry={() => ejectOp.run({ kind: "eject", device: d.path })}
          />
        </div>
      )}
      {op && (
        <OperationBar
          op={op}
          now={now}
          onCancel={() => {
            opCancelling(d.path);
            cancelOperation(d.path).catch(() => {});
          }}
        />
      )}
      <div className="flex-1 overflow-auto p-6">
        {tab === "overview" && <Overview view={view} />}
        {tab === "check" && (
          <FakeCheckTab
            view={view}
            onReformat={(m) => {
              setFormatLimit(m);
              setTab("format");
            }}
          />
        )}
        {tab === "health" && <HealthTab view={view} />}
        {tab === "bench" && <BenchTab view={view} />}
        {tab === "format" && (
          <>
            <FormatTab
              key={formatLimit ?? "sem-limite"}
              view={view}
              initialMaxBytes={formatLimit}
              busy={!!op}
              onSubmit={(req) =>
                formatOp.run(req, {
                  title: "Formatar: apaga tudo",
                  lines: d.partitions.length
                    ? d.partitions.map((p) => `${p.path} (${p.fstype ?? "-"}${p.label ? `, “${p.label}”` : ""}) será apagado`)
                    : [`Todo o conteúdo de ${d.path} será apagado`],
                })
              }
            />
            {formatOp.last("format")?.ok && <p className="mt-4" style={{ color: "var(--ok)" }}>Formatado.</p>}
            {formatOp.last("format") && !formatOp.last("format")!.ok && (
              <p className="mt-4 whitespace-pre-wrap" style={{ color: "var(--bad)" }}>Falhou: {formatOp.last("format")!.error}</p>
            )}
            {formatOp.error && <p className="mt-4" style={{ color: "var(--bad)" }}>{formatOp.error}</p>}
          </>
        )}
        {tab === "history" && <HistoryTab view={view} />}
      </div>
      {formatOp.confirm && <ConfirmDialog token={view.confirmation_token} confirmLabel="Formatar" {...formatOp.confirm} />}
    </div>
  );
}
