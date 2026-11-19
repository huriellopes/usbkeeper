import { useState } from "react";
import type { DeviceView } from "../../lib/bindings/DeviceView";
import type { WipeReport } from "../../lib/bindings/WipeReport";
import { duration, humanBytes } from "../../lib/format";
import { GUIDE } from "../../lib/guideLinks";
import { HelpButton } from "../help/HelpButton";
import { ConfirmDialog } from "../ops/ConfirmDialog";
import { useOperation } from "../ops/useOperation";

type Method = "zeros" | "random" | "discard";
const METHODS: { id: Method; label: string; hint: string }[] = [
  { id: "zeros", label: "Sobrescrever com zeros", hint: "Padrão. Em HD, uma passada basta." },
  { id: "random", label: "Sobrescrever com dados aleatórios", hint: "Não deixa padrão reconhecível." },
  { id: "discard", label: "Descartar (TRIM)", hint: "Muito rápido em SSD/NVMe; muitas pontes USB e pendrives não aceitam." },
];

export function WipeTab({ view, onFormat }: { view: DeviceView; onFormat: () => void }) {
  const op = useOperation(view);
  const d = view.device;
  const [method, setMethod] = useState<Method>("zeros");
  const [passes, setPasses] = useState(1);
  const [verify, setVerify] = useState(false);
  const discard = method === "discard";
  const last = op.last("wipe");
  const report = (last?.result as { report?: WipeReport } | null)?.report;
  const discarded = last?.ok && (last.result as { method?: string } | null)?.method === "discard";
  const estimate = duration((d.size_bytes * (passes + (verify ? 1 : 0))) / 30e6);
  return (
    <div className="flex max-w-2xl flex-col gap-4">
      <p className="text-sm">
        Apaga <strong>tudo</strong> para vender, doar ou descartar. Formatar não basta: os dados continuam recuperáveis.
        <HelpButton topic={GUIDE.wipe} label="apagar com segurança" />
      </p>
      <p className="tone-warn rounded px-3 py-2 text-sm">
        Em pendrive, cartão e SSD, sobrescrever não garante apagar as áreas de reserva do controlador. Para garantia total,
        use criptografia desde o início.
      </p>
      <div role="radiogroup" aria-label="Método" className="flex flex-col gap-2">
        {METHODS.map((m) => (
          <button key={m.id} type="button" role="radio" aria-checked={method === m.id} onClick={() => setMethod(m.id)}
            className="rounded-lg px-3 py-2 text-left text-sm"
            style={{ border: `1px solid ${method === m.id ? "var(--accent)" : "var(--line)"}` }}>
            <strong>{m.label}</strong>
            <span className="block" style={{ color: "var(--muted)" }}>{m.hint}</span>
          </button>
        ))}
      </div>
      <div className="flex flex-wrap items-center gap-4 text-sm">
        <label className="flex items-center gap-2">
          Passadas
          <select aria-label="Passadas" value={passes} disabled={discard} onChange={(e) => setPasses(Number(e.target.value))} className="rounded-md px-2 py-1">
            {[1, 2, 3].map((n) => <option key={n} value={n}>{n}</option>)}
          </select>
        </label>
        <label className="flex items-center gap-2">
          <input type="checkbox" checked={verify} disabled={discard} onChange={(e) => setVerify(e.target.checked)} aria-label="Verificar no fim" />
          Verificar no fim (relê tudo)
        </label>
        {!discard && <span style={{ color: "var(--muted)" }}>Estimativa a 30 MB/s: ~{estimate}</span>}
      </div>
      <button type="button" disabled={!!op.running || view.is_system}
        onClick={() => op.run(
          { kind: "wipe", device: d.path, random: method === "random", passes, verify, discard },
          {
            title: "Apagar tudo: irreversível",
            lines: [
              `Todo o conteúdo de ${d.path} (${humanBytes(d.size_bytes)}) será destruído.`,
              ...d.partitions.map((p) => `${p.path} (${p.fstype ?? "-"}${p.label ? `, “${p.label}”` : ""}) será apagado`),
            ],
          },
        )}
        className="self-start rounded-md px-4 py-1.5 font-semibold disabled:opacity-40" style={{ background: "var(--bad)", color: "white" }}>
        Apagar tudo…
      </button>
      {op.error && <p style={{ color: "var(--bad)" }}>{op.error}</p>}
      {last && !last.ok && <p className="whitespace-pre-wrap" style={{ color: "var(--bad)" }}>Falhou: {last.error}</p>}
      {(report || discarded) && (
        <section className="flex flex-col gap-2 rounded-xl p-4 text-sm" style={{ border: "1px solid var(--line)" }}>
          {discarded && <p>✔ Descartado (TRIM).</p>}
          {report?.verified === true && <p>✔ Apagado e verificado: só restou o padrão gravado.</p>}
          {report?.verified === false && <p style={{ color: "var(--bad)" }}>✘ A verificação encontrou {report.mismatched_bytes} bytes diferentes: partes do disco não aceitaram a gravação.</p>}
          {report && report.verified === null && <p>✔ Apagado ({report.passes} passada(s)).</p>}
          <button type="button" onClick={onFormat} className="self-start rounded-md px-3 py-1" style={{ border: "1px solid var(--line)" }}>
            Formatar agora
          </button>
        </section>
      )}
      {op.confirm && <ConfirmDialog token={view.confirmation_token} confirmLabel="Apagar" {...op.confirm} />}
    </div>
  );
}
