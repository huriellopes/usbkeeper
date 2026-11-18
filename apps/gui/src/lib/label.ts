// Espelho de crates/core/src/format.rs (normalize_label): a CLI valida de
// novo; aqui é só para o usuário ver o erro enquanto digita.
export type Fs = "fat32" | "exfat" | "ntfs" | "ext4";

export const FS: { id: Fs; hint: string; max: number }[] = [
  { id: "exfat", hint: "Windows/macOS/Linux, sem limite de 4 GB; melhor para pendrive", max: 15 },
  { id: "fat32", hint: "compatível com tudo (TV, som, carro, BIOS); arquivos até 4 GB", max: 11 },
  { id: "ntfs", hint: "Windows; leitura/escrita no Linux, só leitura no macOS", max: 32 },
  { id: "ext4", hint: "somente Linux; permissões e journaling", max: 16 },
];

const FORBIDDEN = "\"*+,./:;<=>?[\\]|";
const bytes = (s: string) => new TextEncoder().encode(s).length;

export function validateLabel(fs: Fs, raw: string): { value: string | null; error: string | null } {
  let label = raw.trim();
  if (!label) return { value: null, error: null };
  if (fs === "fat32") {
    const bad = [...label].find((c) => FORBIDDEN.includes(c));
    if (bad) return { value: null, error: `o rótulo FAT32 não pode conter '${bad}'` };
    if (![...label].every((c) => c.charCodeAt(0) < 128)) {
      return { value: null, error: "o rótulo FAT32 deve usar só caracteres ASCII (sem acentos)" };
    }
    label = label.toUpperCase();
  }
  const max = FS.find((f) => f.id === fs)!.max;
  if (bytes(label) > max) {
    return { value: null, error: `rótulo grande demais para ${fs}: ${bytes(label)} bytes (máximo ${max})` };
  }
  return { value: label, error: null };
}
