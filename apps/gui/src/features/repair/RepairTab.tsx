import type { DeviceView } from "../../lib/bindings/DeviceView";
import type { RepairReport } from "../../lib/bindings/RepairReport";
import type { RepairStatus } from "../../lib/bindings/RepairStatus";
import { GUIDE } from "../../lib/guideLinks";
import { HelpButton } from "../help/HelpButton";
import { ConfirmDialog } from "../ops/ConfirmDialog";
import { useOperation } from "../ops/useOperation";

function statusLabel(s: RepairStatus, checkOnly: boolean): { text: string; tone: string } {
  switch (s) {
    case "clean": return { text: "sem problemas", tone: "ok" };
    case "fixed": return { text: "corrigido", tone: "ok" };
    case "errors_remain": return checkOnly ? { text: "tem problemas: rode o reparo", tone: "warn" } : { text: "ainda tem erros", tone: "bad" };
    case "unsupported": return { text: "não verificado", tone: "muted" };
    case "failed": return { text: "a ferramenta falhou", tone: "bad" };
  }
}

const btn = { border: "1px solid var(--line)" };

export function RepairTab({ view }: { view: DeviceView }) {
  const op = useOperation(view);
  const d = view.device;
  const last = op.last("repair");
  const report = (last?.result as { report?: RepairReport } | null)?.report;
  return (
    <div className="flex flex-col gap-4">
      <p className="text-sm" style={{ color: "var(--muted)" }}>
        Confere a estrutura dos sistemas de arquivos (FAT32, exFAT, NTFS e ext4) e corrige inconsistências deixadas por
        remoções sem segurança ou quedas de energia.
        <HelpButton topic={GUIDE.repair} label="verificar e reparar" />
      </p>
      <div className="flex flex-wrap gap-2">
        <button type="button" disabled={!!op.running} onClick={() => op.run({ kind: "repair", device: d.path, check: true })}
          className="rounded-md px-3 py-1 disabled:opacity-40" style={btn}>
          Verificar (não altera nada)
        </button>
        <button type="button" disabled={!!op.running}
          onClick={() => op.run({ kind: "repair", device: d.path, check: false }, {
            title: "Reparar sistemas de arquivos",
            lines: [
              "Corrige as inconsistências e verifica de novo em seguida.",
              "Pedaços de arquivos danificados podem ir para pastas como FSCK0000.REC (FAT/exFAT) ou lost+found (ext4).",
              "Faça backup do que ainda abre antes de reparar.",
            ],
          })}
          className="rounded-md px-3 py-1 disabled:opacity-40" style={btn}>
          Reparar…
        </button>
      </div>
      {op.error && <p style={{ color: "var(--bad)" }}>{op.error}</p>}
      {last && !last.ok && <p className="whitespace-pre-wrap" style={{ color: "var(--bad)" }}>Falhou: {last.error}</p>}
      {report && (
        <section className="flex flex-col gap-3">
          {report.partitions.map((p) => {
            const s = statusLabel(p.status, report.check_only);
            return (
              <div key={p.path} className="rounded-xl p-3 text-sm" style={btn}>
                <div className="flex flex-wrap items-center gap-2">
                  <strong>{p.path}</strong>
                  <span style={{ color: "var(--muted)" }}>{p.fstype ?? "?"}</span>
                  <span className={`badge tone-${s.tone}`}>{s.text}</span>
                </div>
                {p.note && <p className="mt-1">{p.note}</p>}
                {p.log.length > 0 && (
                  <details className="mt-1" open={p.status !== "clean"}>
                    <summary className="cursor-pointer" style={{ color: "var(--muted)" }}>Saída da ferramenta</summary>
                    <pre className="mt-1 whitespace-pre-wrap text-xs">{p.log.join("\n")}</pre>
                  </details>
                )}
              </div>
            );
          })}
          {report.table_warning && <p className="tone-warn rounded px-2 py-1 text-sm">⚠ {report.table_warning}</p>}
        </section>
      )}
      {op.confirm && <ConfirmDialog token={view.confirmation_token} confirmLabel="Reparar" {...op.confirm} />}
    </div>
  );
}
