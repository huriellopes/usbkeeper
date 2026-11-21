import { useState } from "react";
import { mountDevice, openFolder } from "../../lib/api";
import type { DeviceView } from "../../lib/bindings/DeviceView";

const MOUNTABLE = ["vfat", "exfat", "ntfs", "ntfs3", "ext2", "ext3", "ext4"];
const btn = { border: "1px solid var(--line)" };
/** Ponto de montagem que dá para abrir: `/media/…` ou a raiz de uma letra (`E:\`). */
const openable = (m: string) => m.startsWith("/") || /^[A-Za-z]:\\$/.test(m);

/** Montar (para aparecer no gerenciador de arquivos) ou abrir a pasta já montada. */
export function MountButton({ view }: { view: DeviceView }) {
  const d = view.device;
  const current = [...d.mountpoints, ...d.partitions.flatMap((p) => p.mountpoints)].filter(openable);
  const [justMounted, setJustMounted] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const mountpoints = current.length > 0 ? current : justMounted;
  const canMount = d.partitions.some((p) => p.mountpoints.length === 0 && MOUNTABLE.includes(p.fstype ?? ""));
  if (mountpoints.length === 0 && !canMount) return null;
  return (
    <div className="flex flex-col items-end gap-1">
      {mountpoints.length > 0 ? (
        <button type="button" title={mountpoints[0]} onClick={() => openFolder(d.path, mountpoints[0]).catch((e) => setError(String(e)))}
          className="rounded-md px-3 py-1 text-sm" style={btn}>
          Abrir pasta
        </button>
      ) : (
        <button type="button" disabled={busy} title="Monta para aparecer no gerenciador de arquivos"
          onClick={() => {
            setBusy(true);
            setError(null);
            mountDevice(d.path)
              .then(setJustMounted)
              .catch((e) => setError(String(e)))
              .finally(() => setBusy(false));
          }}
          className="rounded-md px-3 py-1 text-sm disabled:opacity-40" style={btn}>
          Montar
        </button>
      )}
      {error && <span className="max-w-xs text-xs" style={{ color: "var(--bad)" }}>{error}</span>}
    </div>
  );
}
