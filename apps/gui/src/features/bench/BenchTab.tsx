import type { DeviceView } from "../../lib/bindings/DeviceView";
import type { BenchResult } from "../../lib/bindings/BenchResult";
import { ConfirmDialog } from "../ops/ConfirmDialog";
import { useOperation } from "../ops/useOperation";
import { GUIDE } from "../../lib/guideLinks";
import { HelpButton } from "../help/HelpButton";

const btn = { border: "1px solid var(--line)" };

export function BenchTab({ view }: { view: DeviceView }) {
  const op = useOperation(view);
  const last = op.last("bench");
  const r = (last?.result as { bench?: BenchResult } | null)?.bench;
  const n = (x: number | null | undefined, unit: string, digits = 1) => (x == null ? "—" : `${x.toFixed(digits)} ${unit}`);
  const tiles: [string, string][] = r
    ? [
        ["Leitura sequencial", n(r.seq_read_mbps, "MB/s")],
        ["Leitura 4K", n(r.rand_read_iops, "IOPS", 0)],
        ["Escrita sequencial", n(r.seq_write_mbps, "MB/s")],
        ["Escrita 4K", n(r.rand_write_iops, "IOPS", 0)],
      ]
    : [];
  return (
    <div className="flex flex-col gap-4">
      <div className="flex gap-2">
        <button type="button" disabled={!!op.running} onClick={() => op.run({ kind: "bench", device: view.device.path, write: false })} className="rounded-md px-3 py-1 disabled:opacity-40" style={btn}>
          Medir leitura
        </button>
        <button
          type="button"
          disabled={!!op.running}
          onClick={() =>
            op.run(
              { kind: "bench", device: view.device.path, write: true },
              { title: "Medir escrita: sobrescreve parte do disco", lines: ["Grava ~256 MB no início, no meio e no fim do disco."] },
            )
          }
          className="rounded-md px-3 py-1 disabled:opacity-40"
          style={btn}
        >
          Medir leitura e escrita
        </button>
        <HelpButton topic={GUIDE.speed} label="velocidade" />
      </div>
      {op.error && <p style={{ color: "var(--bad)" }}>{op.error}</p>}
      {last && !last.ok && <p style={{ color: "var(--bad)" }}>Falhou: {last.error}</p>}
      {r && (
        <dl className="grid grid-cols-2 gap-4">
          {tiles.map(([k, v]) => (
            <div key={k} className="rounded-xl p-4" style={btn}>
              <dt className="text-sm" style={{ color: "var(--muted)" }}>{k}</dt>
              <dd className="text-2xl font-semibold tabular-nums">{v}</dd>
            </div>
          ))}
        </dl>
      )}
      {op.confirm && <ConfirmDialog token={view.confirmation_token} confirmLabel="Iniciar" {...op.confirm} />}
    </div>
  );
}
