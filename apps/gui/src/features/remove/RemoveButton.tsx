import type { DeviceView } from "../../lib/bindings/DeviceView";
import { useOperation } from "../ops/useOperation";

/** Desmonta e desliga. Bloqueado enquanto houver operação no disco. */
export function RemoveButton({ view, compact = false }: { view: DeviceView; compact?: boolean }) {
  const op = useOperation(view);
  const busy = !!op.running;
  const title = busy
    ? "Aguarde: há uma operação em andamento neste disco"
    : "Desmonta e desliga para você poder tirar o dispositivo";
  return (
    <button
      type="button"
      disabled={busy}
      title={title}
      aria-label={compact ? `Remover com segurança ${view.device.name}` : undefined}
      onClick={(e) => {
        e.stopPropagation();
        op.run({ kind: "eject", device: view.device.path });
      }}
      className={compact ? "rounded-md px-2 py-0.5 text-sm disabled:opacity-40" : "rounded-md px-3 py-1 text-sm font-medium disabled:opacity-40"}
      style={{ border: "1px solid var(--line)" }}
    >
      {compact ? "⏏" : "⏏ Remover com segurança"}
    </button>
  );
}
