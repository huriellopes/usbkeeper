import { useState } from "react";
import { useApp } from "../../lib/store";
import { advise } from "../../lib/api";
import type { Advice } from "../../lib/bindings/Advice";
import type { BigFiles } from "../../lib/bindings/BigFiles";
import type { Usage } from "../../lib/bindings/Usage";
import { GUIDE } from "../../lib/guideLinks";
import { HelpButton } from "../help/HelpButton";

const USAGES: { id: Usage; label: string }[] = [
  { id: "mixed", label: "Computadores variados (Windows, Mac e Linux)" },
  { id: "windows_only", label: "Só Windows" },
  { id: "linux_only", label: "Só Linux" },
  { id: "appliance", label: "TV, som, carro ou aparelho antigo" },
  { id: "console", label: "Videogame" },
];
const BIG: { id: BigFiles; label: string }[] = [
  { id: "yes", label: "Sim" },
  { id: "no", label: "Não" },
  { id: "unknown", label: "Não sei" },
];

function Choice<T extends string>({ name, options, value, onChange }: {
  name: string; options: { id: T; label: string }[]; value: T | null; onChange: (v: T) => void;
}) {
  return (
    <div role="radiogroup" aria-label={name} className="flex flex-wrap gap-2">
      {options.map((o) => (
        <button key={o.id} type="button" role="radio" aria-checked={value === o.id} onClick={() => onChange(o.id)}
          className="rounded-lg px-3 py-1.5 text-sm"
          style={{ border: `1px solid ${value === o.id ? "var(--accent)" : "var(--line)"}`, background: value === o.id ? "var(--bg)" : "transparent" }}>
          {o.label}
        </button>
      ))}
    </div>
  );
}

const TABLE: Record<string, string> = { mbr: "MBR", gpt: "GPT" };

/** "Me ajude a escolher": duas perguntas → recomendação explicada. */
export function AdviceCard({ device, onApply }: { device: string; onApply: (a: Advice) => void }) {
  const ext4 = useApp((s) => s.platform.ext4);
  const [usage, setUsage] = useState<Usage | null>(null);
  const [big, setBig] = useState<BigFiles>("unknown");
  const [advice, setAdvice] = useState<Advice | null>(null);
  const [error, setError] = useState<string | null>(null);
  return (
    <section className="flex flex-col gap-3 rounded-xl p-4" style={{ border: "1px solid var(--line)" }}>
      <h3 className="font-semibold">
        Qual formato escolher?
        <HelpButton topic={GUIDE.filesystems} label="sistemas de arquivos" />
      </h3>
      <p className="text-sm">Onde você vai usar?</p>
      <Choice name="Onde vai usar" options={USAGES.filter((u) => u.id !== "linux_only" || ext4)} value={usage} onChange={(v) => { setUsage(v); setAdvice(null); }} />
      <p className="text-sm">Vai guardar arquivos maiores que 4 GB (filmes, imagens ISO, backups)?</p>
      <Choice name="Arquivos maiores que 4 GB" options={BIG} value={big} onChange={(v) => { setBig(v); setAdvice(null); }} />
      <button type="button" disabled={!usage} className="self-start rounded-md px-3 py-1 disabled:opacity-40" style={{ border: "1px solid var(--line)" }}
        onClick={() => usage && advise(device, usage, big).then(setAdvice).catch((e) => setError(String(e)))}>
        Ver recomendação
      </button>
      {error && <p className="text-sm" style={{ color: "var(--bad)" }}>{error}</p>}
      {advice && (
        <div className="flex flex-col gap-2 rounded-lg p-3" style={{ background: "var(--bg)" }}>
          <p className="font-semibold">
            Recomendado: {advice.filesystem} em {TABLE[advice.table] ?? advice.table}
            {advice.max_bytes != null && `, usando só os primeiros ${Math.floor(advice.max_bytes / 1e6)} MB`}
          </p>
          <ul className="text-sm">{advice.reasons.map((r) => <li key={r}>• {r}</li>)}</ul>
          {advice.warnings.length > 0 && (
            <ul className="text-sm">{advice.warnings.map((w) => <li key={w} className="tone-warn rounded px-2 py-1">⚠ {w}</li>)}</ul>
          )}
          <button type="button" onClick={() => onApply(advice)} className="self-start rounded-md px-3 py-1 font-semibold" style={{ background: "var(--accent)", color: "white" }}>
            Usar esta configuração
          </button>
        </div>
      )}
    </section>
  );
}
