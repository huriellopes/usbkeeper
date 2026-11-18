import { useEffect, useState } from "react";
import type { DeviceView } from "../../lib/bindings/DeviceView";
import type { Holder } from "../../lib/bindings/Holder";
import type { Refusal } from "../../lib/bindings/Refusal";
import { deviceHolders } from "../../lib/api";
import { humanBytes } from "../../lib/format";

export function refusalText(r: Refusal): string {
  switch (r.kind) {
    case "system_disk":
      return `É o disco do sistema (montado em ${r.mountpoint}); nunca será alvo.`;
    case "not_usb":
      return `Não é USB (barramento ${r.transport}).`;
    case "mounted":
      return `Está montado em ${r.mountpoints.join(", ")}; será desmontado antes.`;
    case "read_only":
      return "Está protegido contra gravação.";
    case "empty":
      return "Sem mídia inserida.";
  }
}

export function Overview({ view }: { view: DeviceView }) {
  const d = view.device;
  const [holders, setHolders] = useState<Holder[]>([]);
  useEffect(() => {
    deviceHolders(d.path).then(setHolders).catch(() => setHolders([]));
  }, [d.path]);
  const rows: [string, string][] = [
    ["Caminho", d.path],
    ["Tamanho", humanBytes(d.size_bytes)],
    ["Barramento", d.transport.toUpperCase()],
    ["Número de série", d.serial ?? "-"],
    ["Tabela", d.partition_table ?? "-"],
  ];
  if (d.usb) {
    rows.push(["USB", `${d.usb.vendor_id}:${d.usb.product_id} · declara ${d.usb.usb_version ?? "?"} · negociou ${d.usb.speed}`]);
  }
  return (
    <div className="flex flex-col gap-5">
      <table className="w-full text-sm">
        <tbody>
          {rows.map(([k, v]) => (
            <tr key={k}>
              <th className="w-48 py-1 text-left font-normal" style={{ color: "var(--muted)" }}>{k}</th>
              <td>{v}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {d.partitions.length > 0 && (
        <section>
          <h3 className="mb-1 font-semibold">Partições</h3>
          <ul className="text-sm">
            {d.partitions.map((p) => (
              <li key={p.path}>
                {p.path} · {humanBytes(p.size_bytes)} · {p.fstype ?? "-"} {p.label ? `“${p.label}”` : ""} {p.mountpoints.join(", ")}
              </li>
            ))}
          </ul>
        </section>
      )}
      {holders.length > 0 && (
        <section>
          <h3 className="mb-1 font-semibold">Em uso por</h3>
          <ul className="text-sm">
            {holders.map((h) => (
              <li key={h.pid}>PID {h.pid} {h.command} — {h.path}</li>
            ))}
          </ul>
        </section>
      )}
      <section>
        <h3 className="mb-1 font-semibold">Operações destrutivas</h3>
        {view.refusals.length === 0 ? (
          <p className="text-sm">
            Pode ser alvo. A confirmação pedirá: <code>{view.confirmation_token}</code>
          </p>
        ) : (
          <ul className="text-sm">
            {view.refusals.map((r, i) => (
              <li key={i}>• {refusalText(r)}</li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
