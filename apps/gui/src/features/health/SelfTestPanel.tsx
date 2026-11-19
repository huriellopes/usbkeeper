import type { DeviceView } from "../../lib/bindings/DeviceView";
import type { SelfTestEntry } from "../../lib/bindings/SelfTestEntry";
import type { SelfTestState } from "../../lib/bindings/SelfTestState";
import { useOperation } from "../ops/useOperation";

const btn = { border: "1px solid var(--line)" };

type Result = { verdict?: string; result?: SelfTestEntry | null; state?: SelfTestState };

/** Autoteste no firmware do disco (não altera dados). */
export function SelfTestPanel({ view }: { view: DeviceView }) {
  const op = useOperation(view);
  const last = op.last("selftest");
  const res = (last?.result ?? null) as Result | null;
  return (
    <section className="flex flex-col gap-2">
      <h3 className="font-semibold">Autoteste do disco</h3>
      <p className="text-sm" style={{ color: "var(--muted)" }}>
        O próprio HD/SSD se testa. Não altera dados. Muitas pontes USB e pendrives não suportam.
      </p>
      <div className="flex flex-wrap gap-2">
        <button type="button" disabled={!!op.running} onClick={() => op.run({ kind: "selftest", device: view.device.path, long: false })}
          className="rounded-md px-3 py-1 disabled:opacity-40" style={btn}>
          Autoteste curto (~2 min)
        </button>
        <button type="button" disabled={!!op.running} onClick={() => op.run({ kind: "selftest", device: view.device.path, long: true })}
          className="rounded-md px-3 py-1 disabled:opacity-40" style={btn}>
          Autoteste longo (pode levar horas)
        </button>
      </div>
      {op.error && <p style={{ color: "var(--bad)" }}>{op.error}</p>}
      {last && !last.ok && <p className="whitespace-pre-wrap" style={{ color: "var(--bad)" }}>Falhou: {last.error}</p>}
      {res?.verdict && (
        <p className={`rounded px-2 py-1 text-sm tone-${res.verdict === "passed" ? "ok" : res.verdict === "failed" ? "bad" : "muted"}`}>
          {res.verdict === "passed" && `Aprovado: ${res.result?.status ?? ""}`}
          {res.verdict === "failed" && `REPROVADO: ${res.result?.status ?? ""}. Copie seus dados e substitua o disco.`}
          {res.verdict === "unknown" && "Terminou, mas o resultado não apareceu no log do disco."}
        </p>
      )}
      {res?.state && res.state.log.length > 0 && (
        <table className="w-full text-left text-sm">
          <thead><tr><th>Teste</th><th>Resultado</th><th>Horas de uso</th></tr></thead>
          <tbody>
            {res.state.log.slice(0, 5).map((e, i) => (
              <tr key={i}><td>{e.kind}</td><td>{e.passed === false ? "✘ " : e.passed ? "✔ " : ""}{e.status}</td><td>{e.hours ?? "-"}</td></tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
