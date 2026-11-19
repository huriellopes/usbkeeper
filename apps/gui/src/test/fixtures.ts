import type { DeviceView } from "../lib/bindings/DeviceView";

/** DeviceView de teste: pendrive USB 3 negociando USB 2, série ABC. */
export function view(over: Partial<DeviceView> & { model?: string; path?: string } = {}): DeviceView {
  const { model, path: maybePath, ...rest } = over;
  const path = maybePath ?? "/dev/sdb";
  return {
    device: {
      name: path.replace("/dev/", ""), path, size_bytes: 64_000_000_000, transport: "usb", removable: true,
      hotplug: true, read_only: false, vendor: null, model: model ?? "Pen", serial: "ABC",
      partition_table: "dos", logical_sector: 512, physical_sector: 512, system_disk: false, mountpoints: [], partitions: [],
      usb: { vendor_id: "0951", product_id: "1666", manufacturer: null, product: null, usb_version: "3.20", speed: "high" },
    },
    refusals: [], is_system: false, confirmation_token: "ABC", history_key: "0951:1666:ABC",
    last_fake_verdict: null, last_health_verdict: null, suggested_max_bytes: null, ...rest,
  };
}
