import { useEffect } from "react";
import { useApp } from "../../lib/store";

const TIMEOUT_MS = 15_000;

/** Aviso no topo da janela; some em 15 s ou ao clicar. */
export function Notice() {
  const notice = useApp((s) => s.notice);
  const dismiss = useApp((s) => s.dismissNotice);
  useEffect(() => {
    if (!notice) return;
    const t = window.setTimeout(dismiss, TIMEOUT_MS);
    return () => window.clearTimeout(t);
  }, [notice, dismiss]);
  if (!notice) return null;
  return (
    <div role="status" onClick={dismiss} className={`cursor-pointer px-6 py-2 text-sm font-medium tone-${notice.tone}`}>
      {notice.text} <span style={{ opacity: 0.7 }}>(clique para fechar)</span>
    </div>
  );
}
