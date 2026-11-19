import type { Holder } from "../../lib/bindings/Holder";
import { GUIDE } from "../../lib/guideLinks";
import { HelpButton } from "../help/HelpButton";

export function InUseCard({ error, holders, onRetry }: { error: string; holders: Holder[]; onRetry: () => void }) {
  return (
    <section role="alert" className="rounded-xl p-4 text-sm" style={{ border: "1px solid var(--warn)", background: "var(--warn-bg)" }}>
      <h3 className="font-semibold">
        Não dá para remover ainda
        <HelpButton topic={GUIDE.remove} label="remover com segurança" />
      </h3>
      {holders.length > 0 ? (
        <>
          <p className="mt-1">Estes programas estão usando o dispositivo:</p>
          <ul className="mt-1">
            {holders.map((h) => (
              <li key={h.pid}>
                • <strong>{h.command}</strong> (PID {h.pid}): {h.path}
              </li>
            ))}
          </ul>
          <p className="mt-2">
            Feche-os e tente de novo. Muitas vezes é uma janela do gerenciador de arquivos aberta no pendrive, ou um
            terminal dentro de uma pasta dele.
          </p>
        </>
      ) : (
        <p className="mt-1 whitespace-pre-wrap">{error}</p>
      )}
      <button type="button" onClick={onRetry} className="mt-3 rounded-md px-3 py-1" style={{ border: "1px solid var(--line)", background: "var(--panel)" }}>
        Tentar de novo
      </button>
    </section>
  );
}
