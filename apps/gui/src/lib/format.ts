export function humanBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit++;
  }
  return unit === 0 ? `${bytes} B` : `${value.toFixed(1)} ${units[unit]}`;
}

export function duration(secs: number): string {
  const s = Math.max(0, Math.floor(secs));
  if (s >= 3600) return `${Math.floor(s / 3600)}h${String(Math.floor((s % 3600) / 60)).padStart(2, "0")}min`;
  if (s >= 60) return `${Math.floor(s / 60)}min${String(s % 60).padStart(2, "0")}s`;
  return `${s}s`;
}

const LABELS: Record<string, string> = {
  genuine: "genuíno", fake: "FALSO", damaged: "defeito", good: "saudável", warning: "atenção",
  failing: "FALHANDO", unknown: "sem SMART", clean: "sem erros", unreadable: "ILEGÍVEL", fixed: "corrigido", errors: "com erros",
};
export const verdictLabel = (v?: string | null) => (v ? LABELS[v] ?? v : "");

export function verdictTone(v?: string | null): "ok" | "warn" | "bad" | "muted" {
  if (v === "genuine" || v === "good" || v === "clean" || v === "fixed") return "ok";
  if (v === "warning" || v === "damaged") return "warn";
  if (v === "fake" || v === "failing" || v === "unreadable" || v === "errors") return "bad";
  return "muted";
}
