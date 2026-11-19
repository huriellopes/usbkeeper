import { useEffect, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import type { DeviceView } from "../../lib/bindings/DeviceView";
import type { Event } from "../../lib/bindings/Event";
import { historyEvents, saveReport } from "../../lib/api";
import { verdictLabel, verdictTone } from "../../lib/format";

const KIND: Record<string, string> = { fake_check: "fake-check", bench: "velocidade", health: "SMART", scan: "varredura", repair: "reparo", selftest: "autoteste", wipe: "apagamento" };

export function HistoryTab({ view }: { view: DeviceView }) {
  const [events, setEvents] = useState<Event[]>([]);
  const [msg, setMsg] = useState<string | null>(null);
  useEffect(() => {
    historyEvents(view.history_key).then(setEvents).catch((e) => setMsg(String(e)));
  }, [view.history_key]);
  const exportReport = async () => {
    const path = await save({ defaultPath: `usbkeeper-${view.device.name}.html`, filters: [{ name: "HTML", extensions: ["html"] }] });
    if (!path) return;
    saveReport(view.history_key, path)
      .then(() => setMsg(`Laudo salvo em ${path}`))
      .catch((e) => setMsg(String(e)));
  };
  return (
    <div className="flex flex-col gap-3">
      <button type="button" disabled={events.length === 0} onClick={exportReport} className="self-start rounded-md px-3 py-1 disabled:opacity-40" style={{ border: "1px solid var(--line)" }}>
        Salvar laudo HTML…
      </button>
      {msg && <p className="text-sm">{msg}</p>}
      {events.length === 0 ? (
        <p style={{ color: "var(--muted)" }}>Nenhum teste registrado para este dispositivo.</p>
      ) : (
        <table className="w-full text-left text-sm">
          <thead>
            <tr><th>Quando</th><th>Teste</th><th>Resultado</th><th>Detalhes</th></tr>
          </thead>
          <tbody>
            {[...events].reverse().map((e) => (
              <tr key={e.id}>
                <td className="whitespace-nowrap pr-3">{new Date(e.at * 1000).toLocaleString("pt-BR")}</td>
                <td className="pr-3">{KIND[e.kind] ?? e.kind}</td>
                <td className="pr-3">{e.verdict && <span className={`badge tone-${verdictTone(e.verdict)}`}>{verdictLabel(e.verdict)}</span>}</td>
                <td>{e.summary}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
