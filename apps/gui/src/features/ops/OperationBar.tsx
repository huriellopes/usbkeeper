import type { RunningOp } from "../../lib/store";
import { duration, humanBytes } from "../../lib/format";

export function eta(op: RunningOp, now: number): string | null {
  const p = op.progress;
  if (!p || !p.bytes || p.done === 0) return null;
  const secs = (now - op.phaseStartedAt) / 1000;
  if (secs < 0.5) return null;
  const rate = p.done / secs;
  return `${humanBytes(rate)}/s · restam ~${duration((p.total - p.done) / rate)}`;
}

export function OperationBar({ op, onCancel, now = Date.now() }: { op: RunningOp; onCancel: () => void; now?: number }) {
  const p = op.progress;
  const pct = p && p.total > 0 ? Math.min(100, (p.done * 100) / p.total) : 0;
  const extra = eta(op, now);
  return (
    <div role="status" className="flex items-center gap-3 border-b px-6 py-2 text-sm" style={{ borderColor: "var(--line)", background: "var(--bg)" }}>
      <span className="w-56 truncate">{p ? p.label : "aguardando autorização…"}</span>
      <div className="h-2 flex-1 overflow-hidden rounded-full" style={{ background: "var(--line)" }}>
        <div className="h-full" style={{ width: `${pct}%`, background: "var(--accent)" }} />
      </div>
      <span className="w-14 text-right tabular-nums">{pct.toFixed(0)}%</span>
      {extra && <span className="tabular-nums" style={{ color: "var(--muted)" }}>{extra}</span>}
      <button type="button" onClick={onCancel} disabled={op.cancelling} className="rounded-md px-3 py-1" style={{ border: "1px solid var(--line)" }}>
        {op.cancelling ? "Cancelando…" : "Cancelar"}
      </button>
    </div>
  );
}
