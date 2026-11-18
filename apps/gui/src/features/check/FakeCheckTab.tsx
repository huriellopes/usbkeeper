import type { DeviceView } from "../../lib/bindings/DeviceView";
import type { FakeReport } from "../../lib/bindings/FakeReport";
import { duration, humanBytes } from "../../lib/format";
import { ConfirmDialog } from "../ops/ConfirmDialog";
import { useOperation } from "../ops/useOperation";

const card = { border: "1px solid var(--line)" };

export function FakeCheckTab({ view, onReformat }: { view: DeviceView; onReformat: (maxBytes: number) => void }) {
  const op = useOperation(view);
  const d = view.device;
  const last = op.last("fake_check");
  const report = (last?.result as { report?: FakeReport } | null)?.report;
  const parts = d.partitions.map((p) => `${p.path} (${humanBytes(p.size_bytes)}, ${p.fstype ?? "sem sistema de arquivos"}) será apagado`);
  const tone = report?.verdict === "genuine" ? "ok" : report?.verdict === "fake" ? "bad" : "warn";
  const title = report?.verdict === "genuine" ? "GENUÍNO" : report?.verdict === "fake" ? "FALSIFICADO" : "COM DEFEITO";
  return (
    <div className="flex flex-col gap-5">
      <div className="grid gap-3 sm:grid-cols-2">
        <button
          type="button"
          disabled={!!op.running}
          onClick={() =>
            op.run(
              { kind: "fake_check", device: d.path, full: false },
              { title: "Teste rápido de capacidade", lines: ["Grava ~200 blocos de 4 KiB e restaura os originais no fim.", "Tenha backup do que for importante."] },
            )
          }
          className="rounded-xl p-4 text-left disabled:opacity-40"
          style={card}
        >
          <strong>Teste rápido</strong>
          <p className="text-sm" style={{ color: "var(--muted)" }}>Segundos. Mantém os dados. Pega os falsos comuns.</p>
        </button>
        <button
          type="button"
          disabled={!!op.running}
          onClick={() =>
            op.run(
              { kind: "fake_check", device: d.path, full: true },
              {
                title: "Teste COMPLETO: apaga tudo",
                lines: ["Grava e lê o disco inteiro.", `Tempo estimado a 10 MB/s: ~${duration((d.size_bytes * 2) / 10e6)}.`, ...parts],
              },
            )
          }
          className="rounded-xl p-4 text-left disabled:opacity-40"
          style={card}
        >
          <strong>Teste completo</strong>
          <p className="text-sm" style={{ color: "var(--muted)" }}>Definitivo. APAGA o disco. Pode levar horas.</p>
        </button>
      </div>
      {op.error && <p style={{ color: "var(--bad)" }}>{op.error}</p>}
      {last && !last.ok && <p style={{ color: "var(--bad)" }}>Falhou: {last.error}</p>}
      {report && (
        <section className="rounded-xl p-5" style={card}>
          <span className={`badge tone-${tone}`}>{title}</span>
          <p className="mt-3 text-2xl font-semibold">
            {humanBytes(report.real_bytes)} reais de {humanBytes(report.advertised_bytes)}
          </p>
          <div className="mt-2 h-3 overflow-hidden rounded-full" style={{ background: "var(--bad-bg)" }}>
            <div className="h-full" style={{ width: `${(report.real_bytes * 100) / Math.max(1, report.advertised_bytes)}%`, background: "var(--ok)" }} />
          </div>
          {report.wrap_module_bytes != null && (
            <p className="mt-2 text-sm">Dá a volta a cada {humanBytes(report.wrap_module_bytes)}: o que passa disso sobrescreve o início.</p>
          )}
          {report.verdict === "genuine" && report.mode === "quick" && (
            <p className="mt-2 text-sm" style={{ color: "var(--muted)" }}>
              O rápido pega os tipos comuns de falsificação; para certeza absoluta use o completo.
            </p>
          )}
          {report.suggested_max_bytes != null && (
            <button
              type="button"
              className="mt-4 rounded-md px-3 py-1 font-semibold"
              style={{ background: "var(--accent)", color: "white" }}
              onClick={() => onReformat(report.suggested_max_bytes!)}
            >
              Reformatar só a parte real
            </button>
          )}
        </section>
      )}
      {op.confirm && <ConfirmDialog token={view.confirmation_token} confirmLabel="Iniciar" {...op.confirm} />}
    </div>
  );
}
