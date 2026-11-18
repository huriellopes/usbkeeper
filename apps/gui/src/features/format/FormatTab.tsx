import { useState } from "react";
import type { DeviceView } from "../../lib/bindings/DeviceView";
import type { OpRequest } from "../../lib/bindings/OpRequest";
import { FS, type Fs, validateLabel } from "../../lib/label";

type Props = { view: DeviceView; initialMaxBytes?: number; onSubmit: (req: OpRequest) => void; busy: boolean };

export function FormatTab({ view, initialMaxBytes, onSubmit, busy }: Props) {
  const [fs, setFs] = useState<Fs>("exfat");
  const [label, setLabel] = useState("");
  const [table, setTable] = useState<"auto" | "mbr" | "gpt">("auto");
  const [limitMb, setLimitMb] = useState(initialMaxBytes ? String(Math.floor(initialMaxBytes / 1e6)) : "");
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
      <label className="flex flex-col text-sm">
        Sistema de arquivos
        <select aria-label="Sistema de arquivos" value={fs} onChange={(e) => setFs(e.target.value as Fs)} className="mt-1 rounded-md px-2 py-1">
          {FS.map((f) => (
            <option key={f.id} value={f.id}>{f.id}</option>
          ))}
        </select>
        <span className="mt-1" style={{ color: "var(--muted)" }}>{FS.find((f) => f.id === fs)!.hint}</span>
      </label>
      <label className="flex flex-col text-sm">
        Rótulo
        <input aria-label="Rótulo" value={label} onChange={(e) => setLabel(e.target.value)} className="mt-1 rounded-md px-2 py-1" />
        {check.error && <span className="mt-1" style={{ color: "var(--bad)" }}>{check.error}</span>}
      </label>
      <label className="flex flex-col text-sm">
        Tabela
        <select aria-label="Tabela" value={limited ? "mbr" : table} disabled={limited} onChange={(e) => setTable(e.target.value as typeof table)} className="mt-1 rounded-md px-2 py-1">
          <option value="auto">automática (MBR até 2 TiB)</option>
          <option value="mbr">MBR</option>
          <option value="gpt">GPT</option>
        </select>
        {limited && (
          <span className="mt-1" style={{ color: "var(--muted)" }}>Com limite de tamanho, só MBR é seguro num pendrive falsificado.</span>
        )}
      </label>
      <label className="flex flex-col text-sm">
        Usar só os primeiros (MB, opcional)
        <input aria-label="Usar só os primeiros (MB)" inputMode="numeric" value={limitMb} onChange={(e) => setLimitMb(e.target.value)} className="mt-1 rounded-md px-2 py-1" />
        {invalidLimit && <span className="mt-1" style={{ color: "var(--bad)" }}>Informe um número de MB.</span>}
      </label>
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
