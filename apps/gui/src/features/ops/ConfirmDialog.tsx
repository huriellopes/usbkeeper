import { useState } from "react";

type Props = {
  title: string;
  token: string;
  lines: string[];
  confirmLabel: string;
  onConfirm: () => void;
  onClose: () => void;
};

/** Confirmação destrutiva: só libera com o mesmo token que a CLI exige. */
export function ConfirmDialog({ title, token, lines, confirmLabel, onConfirm, onClose }: Props) {
  const [typed, setTyped] = useState("");
  const ok = typed.trim() === token;
  return (
    <div role="dialog" aria-modal="true" aria-label={title} className="fixed inset-0 grid place-items-center p-4" style={{ background: "rgb(0 0 0 / 0.45)" }}>
      <div className="w-full max-w-lg rounded-xl p-5" style={{ background: "var(--panel)", border: "1px solid var(--line)" }}>
        <h2 className="text-lg font-semibold">{title}</h2>
        {lines.length > 0 && (
          <ul className="my-3 text-sm">
            {lines.map((l) => (
              <li key={l}>• {l}</li>
            ))}
          </ul>
        )}
        <label className="mt-2 block text-sm">
          Digite {token} para confirmar
          <input autoFocus value={typed} onChange={(e) => setTyped(e.target.value)} className="mt-1 w-full rounded-md px-2 py-1 font-mono" />
        </label>
        <div className="mt-4 flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded-md px-3 py-1">Voltar</button>
          <button type="button" disabled={!ok} onClick={onConfirm} className="rounded-md px-3 py-1 font-semibold disabled:opacity-40" style={{ background: "var(--bad)", color: "white" }}>
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
