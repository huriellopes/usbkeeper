import { useState } from "react";
import { useApp } from "../../lib/store";
import type { DeviceView } from "../../lib/bindings/DeviceView";
import type { OpRequest } from "../../lib/bindings/OpRequest";
import { FS, type Fs, validateLabel } from "../../lib/label";
import { GUIDE } from "../../lib/guideLinks";
import { HelpButton } from "../help/HelpButton";

/** Valores iniciais do formulário (assistente ou "reformatar só a parte real"). */
export type FormatInitial = { filesystem?: Fs; table?: "mbr" | "gpt"; maxBytes?: number };

type Props = { view: DeviceView; initial?: FormatInitial; onSubmit: (req: OpRequest) => void; busy: boolean };

export function FormatTab({ view, initial = {}, onSubmit, busy }: Props) {
  const ext4 = useApp((s) => s.platform.ext4);
  const [fs, setFs] = useState<Fs>(initial.filesystem ?? "exfat");
  const [label, setLabel] = useState("");
  const [table, setTable] = useState<"auto" | "mbr" | "gpt">(initial.table ?? "auto");
  const [limitMb, setLimitMb] = useState(initial.maxBytes ? String(Math.floor(initial.maxBytes / 1e6)) : "");
  const check = validateLabel(fs, label);
  const maxBytes = limitMb.trim() ? Number(limitMb) * 1e6 : null;
  const limited = maxBytes != null && Number.isFinite(maxBytes) && maxBytes > 0;
  const invalidLimit = limitMb.trim() !== "" && !limited;
  return (
    <form
      className="flex max-w-xl flex-col gap-4"
      onSubmit={(e) => {
        e.preventDefault();
        onSubmit({
          kind: "format",
          device: view.device.path,
          filesystem: fs,
          table: limited || table === "auto" ? null : table,
          label: check.value,
          max_bytes: limited ? maxBytes : null,
        });
      }}
    >
      <div className="flex flex-col text-sm">
        <span>
          Sistema de arquivos
          <HelpButton topic={GUIDE.filesystems} label="sistema de arquivos" />
        </span>
        <select aria-label="Sistema de arquivos" value={fs} onChange={(e) => setFs(e.target.value as Fs)} className="mt-1 rounded-md px-2 py-1">
          {FS.filter((f) => f.id !== "ext4" || ext4).map((f) => (
            <option key={f.id} value={f.id}>{f.id}</option>
          ))}
        </select>
        <span className="mt-1" style={{ color: "var(--muted)" }}>{FS.find((f) => f.id === fs)!.hint}</span>
      </div>
      <label className="flex flex-col text-sm">
        Rótulo
        <input aria-label="Rótulo" value={label} onChange={(e) => setLabel(e.target.value)} className="mt-1 rounded-md px-2 py-1" />
        {check.error && <span className="mt-1" style={{ color: "var(--bad)" }}>{check.error}</span>}
      </label>
      <div className="flex flex-col text-sm">
        <span>
          Tabela
          <HelpButton topic={GUIDE.partitionTable} label="tabela" />
        </span>
        <select aria-label="Tabela" value={limited ? "mbr" : table} disabled={limited} onChange={(e) => setTable(e.target.value as typeof table)} className="mt-1 rounded-md px-2 py-1">
          <option value="auto">automática (MBR até 2 TiB)</option>
          <option value="mbr">MBR</option>
          <option value="gpt">GPT</option>
        </select>
        {limited && (
          <span className="mt-1" style={{ color: "var(--muted)" }}>Com limite de tamanho, só MBR é seguro num pendrive falsificado.</span>
        )}
      </div>
      <div className="flex flex-col text-sm">
        <span>
          Usar só os primeiros (MB, opcional)
          <HelpButton topic={GUIDE.fake} label="usar só os primeiros" />
        </span>
        <input aria-label="Usar só os primeiros (MB)" inputMode="numeric" value={limitMb} onChange={(e) => setLimitMb(e.target.value)} className="mt-1 rounded-md px-2 py-1" />
        {invalidLimit && <span className="mt-1" style={{ color: "var(--bad)" }}>Informe um número de MB.</span>}
      </div>
      <p className="text-sm" style={{ color: "var(--muted)" }}>
        Antes de formatar, leia o que acontece com os dados
        <HelpButton topic={GUIDE.format} label="o que formatar faz" />
      </p>
      <button
        type="submit"
        disabled={busy || !!check.error || invalidLimit || view.refusals.some((r) => r.kind === "system_disk")}
        className="self-start rounded-md px-4 py-1.5 font-semibold disabled:opacity-40"
        style={{ background: "var(--bad)", color: "white" }}
      >
        Formatar…
      </button>
    </form>
  );
}
