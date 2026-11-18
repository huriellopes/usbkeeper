import { useMemo, useState } from "react";
import type { GuideTopic } from "../../lib/bindings/GuideTopic";
import { useApp } from "../../lib/store";

const plain = (html: string) => html.replace(/<[^>]+>/g, " ").toLowerCase();

export function HelpPanel({ topics }: { topics: GuideTopic[] }) {
  const help = useApp((s) => s.help);
  const openHelp = useApp((s) => s.openHelp);
  const closeHelp = useApp((s) => s.closeHelp);
  const [query, setQuery] = useState("");
  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return topics;
    return topics.filter(
      (t) => t.title.toLowerCase().includes(q) || t.summary.toLowerCase().includes(q) || plain(t.html).includes(q),
    );
  }, [query, topics]);
  const current = topics.find((t) => t.id === help.topic) ?? topics[0];
  return (
    <div className="grid h-full" style={{ gridTemplateColumns: "300px 1fr" }}>
      <nav className="flex flex-col gap-2 overflow-auto border-r p-3" style={{ borderColor: "var(--line)" }} aria-label="Tópicos do guia">
        <button type="button" onClick={closeHelp} className="self-start rounded-md px-2 py-1 text-sm" style={{ border: "1px solid var(--line)" }}>
          ← Voltar
        </button>
        <input aria-label="Buscar no guia" placeholder="Buscar no guia…" value={query} onChange={(e) => setQuery(e.target.value)} className="rounded-md px-2 py-1 text-sm" />
        <ul className="flex flex-col gap-1">
          {visible.map((t) => (
            <li key={t.id}>
              <button
                type="button"
                onClick={() => openHelp(t.id)}
                aria-current={t.id === current?.id}
                className="w-full rounded-md px-2 py-1.5 text-left"
                style={{ background: t.id === current?.id ? "var(--line)" : "transparent" }}
              >
                <span className="block text-sm font-medium">{t.title}</span>
                <span className="block text-xs" style={{ color: "var(--muted)" }}>{t.summary}</span>
              </button>
            </li>
          ))}
          {visible.length === 0 && <li className="px-2 text-sm" style={{ color: "var(--muted)" }}>Nada encontrado.</li>}
        </ul>
      </nav>
      {/* Conteúdo compilado no binário (crates/guide), sem HTML bruto: seguro de inserir. */}
      <article className="guide overflow-auto px-8 py-6" dangerouslySetInnerHTML={{ __html: current?.html ?? "" }} />
    </div>
  );
}
