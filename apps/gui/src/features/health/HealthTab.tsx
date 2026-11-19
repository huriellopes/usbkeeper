import type { DeviceView } from "../../lib/bindings/DeviceView";
import type { HealthReport } from "../../lib/bindings/HealthReport";
import type { ScanReport } from "../../lib/bindings/ScanReport";
import { humanBytes, verdictLabel, verdictTone } from "../../lib/format";
import { useOperation } from "../ops/useOperation";
import { GUIDE } from "../../lib/guideLinks";
import { HelpButton } from "../help/HelpButton";

const btn = { border: "1px solid var(--line)" };

export function HealthTab({ view }: { view: DeviceView }) {
  const op = useOperation(view);
  const last = op.last("health");
  const res = last?.result as { smart?: HealthReport; scan?: ScanReport | null } | null;
  const smart = res?.smart;
  return (
    <div className="flex flex-col gap-4">
      <div className="flex gap-2">
        <button type="button" disabled={!!op.running} onClick={() => op.run({ kind: "health", device: view.device.path, scan: false })} className="rounded-md px-3 py-1 disabled:opacity-40" style={btn}>
          Ler SMART
        </button>
        <button type="button" disabled={!!op.running} onClick={() => op.run({ kind: "health", device: view.device.path, scan: true })} className="rounded-md px-3 py-1 disabled:opacity-40" style={btn}>
          SMART + varrer superfície
        </button>
        <HelpButton topic={GUIDE.health} label="saúde e SMART" />
      </div>
      <p className="text-sm" style={{ color: "var(--muted)" }}>Somente leitura: nada é gravado no disco.</p>
      {op.error && <p style={{ color: "var(--bad)" }}>{op.error}</p>}
      {last && !last.ok && <p style={{ color: "var(--bad)" }}>Falhou: {last.error}</p>}
      {smart && (
        <section>
          <span className={`badge tone-${verdictTone(smart.verdict)}`}>{verdictLabel(smart.verdict)}</span>
          <ul className="mt-2 text-sm">
            {smart.notes.map((n) => (
              <li key={n}>• {n}</li>
            ))}
          </ul>
          {smart.attributes.length > 0 && (
            <details className="mt-2">
              <summary className="cursor-pointer" style={{ color: "var(--muted)" }}>Atributos SMART</summary>
              <table className="mt-2 w-full text-left text-sm">
                <thead>
                  <tr><th>ID</th><th>Nome</th><th>Valor</th><th>Pior</th><th>Limite</th><th>Bruto</th></tr>
                </thead>
                <tbody>
                  {smart.attributes.map((a) => (
                    <tr key={a.id}><td>{a.id}</td><td>{a.name}</td><td>{a.value}</td><td>{a.worst}</td><td>{a.threshold}</td><td>{a.raw}</td></tr>
                  ))}
                </tbody>
              </table>
            </details>
          )}
        </section>
      )}
      {res?.scan && (
        <section className="text-sm">
          <h3 className="font-semibold">Varredura</h3>
          {res.scan.unreadable_blocks === 0 ? (
            <p>Nenhum bloco ilegível em {humanBytes(res.scan.scanned_bytes)}.</p>
          ) : (
            <p style={{ color: "var(--bad)" }}>
              {res.scan.unreadable_blocks} blocos de 4 KiB ilegíveis (primeiro em {humanBytes(res.scan.unreadable_offsets[0] ?? 0)}).
            </p>
          )}
          {res.scan.slow_chunks > 0 && <p>{res.scan.slow_chunks} trechos lentos (pior: {res.scan.slowest_chunk_ms} ms).</p>}
        </section>
      )}
    </div>
  );
}
