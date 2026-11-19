import { useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { imageMeta } from "../../lib/api";
import type { DeviceView } from "../../lib/bindings/DeviceView";
import { humanBytes } from "../../lib/format";
import { GUIDE } from "../../lib/guideLinks";
import { HelpButton } from "../help/HelpButton";
import { ConfirmDialog } from "../ops/ConfirmDialog";
import { useOperation } from "../ops/useOperation";

const card = { border: "1px solid var(--line)" };
type Done = { report?: { bytes: number; sha256: string }; verified?: boolean; meta?: { sha256: string }; path?: string; file_bytes?: number };

function Result({ ok, error, result }: { ok?: boolean; error?: string | null; result?: unknown }) {
  if (ok === undefined) return null;
  if (!ok) return <p className="whitespace-pre-wrap text-sm" style={{ color: "var(--bad)" }}>Falhou: {error}</p>;
  const r = (result ?? {}) as Done;
  const sha = r.report?.sha256 ?? r.meta?.sha256;
  return (
    <p className="text-sm" style={{ color: "var(--ok)" }}>
      ✔ {r.path ? `Imagem salva em ${r.path}${r.file_bytes ? ` (${humanBytes(r.file_bytes)})` : ""}` : "Gravado e verificado"}
      {sha && <span className="block break-all text-xs" style={{ color: "var(--muted)" }}>SHA-256 {sha}</span>}
    </p>
  );
}

export function ImageTab({ view }: { view: DeviceView }) {
  const op = useOperation(view);
  const d = view.device;
  const [sha, setSha] = useState("");
  const [error, setError] = useState<string | null>(null);
  const busy = !!op.running;
  const last = (k: string) => op.last(k);

  const createImage = async () => {
    const output = await save({ defaultPath: `${d.name}.img.zst`, filters: [{ name: "Imagem", extensions: ["zst", "img"] }] });
    if (output) op.run({ kind: "image_create", device: d.path, output });
  };
  const restoreImage = async () => {
    const image = await open({ multiple: false, filters: [{ name: "Imagem", extensions: ["zst", "img"] }] });
    if (typeof image !== "string") return;
    const m = (await imageMeta(image).catch(() => null)) as { model?: string; size_bytes?: number; created_at?: string } | null;
    if (m?.size_bytes && m.size_bytes > d.size_bytes) {
      setError(`A imagem tem ${humanBytes(m.size_bytes)} e o disco só ${humanBytes(d.size_bytes)}.`);
      return;
    }
    setError(null);
    op.run({ kind: "image_restore", device: d.path, image }, {
      title: "Restaurar imagem: substitui tudo",
      lines: [
        m ? `Imagem de ${m.model ?? "?"} (${humanBytes(m.size_bytes ?? 0)}), criada em ${m.created_at ?? "?"}` : `Imagem ${image} (sem arquivo .json de informações)`,
        `Todo o conteúdo de ${d.path} será substituído.`,
      ],
    });
  };
  const burnIso = async () => {
    const iso = await open({ multiple: false, filters: [{ name: "ISO", extensions: ["iso", "img"] }] });
    if (typeof iso !== "string") return;
    op.run({ kind: "burn_iso", device: d.path, iso, sha256: sha.trim() || null }, {
      title: "Gravar ISO: apaga tudo",
      lines: [`${iso} será gravada em ${d.path}.`, `Todo o conteúdo de ${d.path} será apagado.`],
    });
  };

  return (
    <div className="flex max-w-2xl flex-col gap-5">
      <p className="text-sm" style={{ color: "var(--muted)" }}>
        Backup do disco inteiro e criação de pendrive de boot.
        <HelpButton topic={GUIDE.image} label="imagem do disco e ISO" />
      </p>
      <section className="flex flex-col gap-2 rounded-xl p-4" style={card}>
        <h3 className="font-semibold">Criar imagem (backup)</h3>
        <p className="text-sm" style={{ color: "var(--muted)" }}>Copia o disco inteiro para um arquivo comprimido, com hash. Só lê o disco.</p>
        <button type="button" disabled={busy} onClick={createImage} className="self-start rounded-md px-3 py-1 disabled:opacity-40" style={card}>Criar imagem…</button>
        <Result ok={last("image_create")?.ok} error={last("image_create")?.error} result={last("image_create")?.result} />
      </section>
      <section className="flex flex-col gap-2 rounded-xl p-4" style={card}>
        <h3 className="font-semibold">Restaurar imagem</h3>
        <p className="text-sm" style={{ color: "var(--muted)" }}>Volta o disco ao estado da imagem e confere o hash no fim. Apaga o que houver nele.</p>
        <button type="button" disabled={busy} onClick={restoreImage} className="self-start rounded-md px-3 py-1 disabled:opacity-40" style={card}>Restaurar imagem…</button>
        {error && <p className="text-sm" style={{ color: "var(--bad)" }}>{error}</p>}
        <Result ok={last("image_restore")?.ok} error={last("image_restore")?.error} result={last("image_restore")?.result} />
      </section>
      <section className="flex flex-col gap-2 rounded-xl p-4" style={card}>
        <h3 className="font-semibold">Gravar ISO de boot</h3>
        <p className="text-sm" style={{ color: "var(--muted)" }}>
          Para instaladores e sistemas “live” (Linux e outros). ISOs do Windows precisam de preparo especial e são recusadas.
        </p>
        <label className="flex flex-col text-sm">
          SHA-256 da ISO (opcional, do site de download)
          <input aria-label="SHA-256 da ISO" value={sha} onChange={(e) => setSha(e.target.value)} placeholder="ou deixe SHA256SUMS na mesma pasta da ISO" className="mt-1 rounded-md px-2 py-1 font-mono text-xs" />
        </label>
        <button type="button" disabled={busy} onClick={burnIso} className="self-start rounded-md px-3 py-1 disabled:opacity-40" style={card}>Gravar ISO…</button>
        <Result ok={last("burn_iso")?.ok} error={last("burn_iso")?.error} result={last("burn_iso")?.result} />
      </section>
      {op.confirm && (
        <ConfirmDialog token={view.confirmation_token} confirmLabel={op.confirm.title.startsWith("Gravar") ? "Gravar" : "Restaurar"} {...op.confirm} />
      )}
      {op.error && <p style={{ color: "var(--bad)" }}>{op.error}</p>}
    </div>
  );
}
