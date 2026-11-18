import type { DeviceView } from "../../lib/bindings/DeviceView";
import { humanBytes, verdictLabel, verdictTone } from "../../lib/format";

type Props = { devices: DeviceView[]; selected: string | null; onSelect: (path: string) => void; hint?: string | null };

export function Sidebar({ devices, selected, onSelect, hint }: Props) {
  const usb = devices.some((v) => v.device.transport === "usb");
  const empty = (
    <div className="p-4 text-sm" style={{ color: "var(--muted)" }}>
      <p>Conecte um pendrive ou HD externo.</p>
      {hint && <pre className="mt-2 whitespace-pre-wrap text-xs">{hint}</pre>}
    </div>
  );
  if (devices.length === 0) return empty;
  return (
    <ul className="flex flex-col gap-1 p-2" aria-label="Discos">
      {devices.map((v) => {
        const d = v.device;
        const active = d.path === selected;
        const slow =
          !!d.usb?.usb_version && Number.parseFloat(d.usb.usb_version) >= 3 && ["low", "full", "high"].includes(d.usb.speed);
        return (
          <li key={d.path}>
            <button
              type="button"
              onClick={() => onSelect(d.path)}
              aria-current={active}
              className="w-full rounded-lg px-3 py-2 text-left"
              style={{
                background: active ? "var(--panel)" : "transparent",
                opacity: v.is_system ? 0.6 : 1,
                border: `1px solid ${active ? "var(--line)" : "transparent"}`,
              }}
            >
              <div className="flex items-center justify-between gap-2">
                <span className="truncate font-medium">
                  {v.is_system && <span aria-label="disco do sistema">🔒 </span>}
                  {d.model ?? d.name}
                </span>
                <span className="text-xs" style={{ color: "var(--muted)" }}>{humanBytes(d.size_bytes)}</span>
              </div>
              <div className="mt-1 flex flex-wrap gap-1 text-xs">
                <span style={{ color: "var(--muted)" }}>{d.path}</span>
                {slow && <span className="badge tone-warn">USB 2.0 ⚠</span>}
                {v.last_fake_verdict && (
                  <span className={`badge tone-${verdictTone(v.last_fake_verdict)}`}>{verdictLabel(v.last_fake_verdict)}</span>
                )}
                {v.last_health_verdict && (
                  <span className={`badge tone-${verdictTone(v.last_health_verdict)}`}>{verdictLabel(v.last_health_verdict)}</span>
                )}
              </div>
            </button>
          </li>
        );
      })}
      {!usb && <li>{empty}</li>}
    </ul>
  );
}
