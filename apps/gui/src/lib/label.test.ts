import { describe, expect, it } from "vitest";
import { validateLabel } from "./label";

// Mesmos casos de crates/core/src/format.rs (normalize_label).
describe("validateLabel", () => {
  it("FAT32: maiúsculas, ASCII, sem proibidos, até 11 bytes", () => {
    expect(validateLabel("fat32", " meu pen ")).toEqual({ value: "MEU PEN", error: null });
    expect(validateLabel("fat32", "a/b").error).toMatch(/não pode conter '\/'/);
    expect(validateLabel("fat32", "câmera").error).toMatch(/ASCII/);
    expect(validateLabel("fat32", "DOZE_LETRAS_").error).toMatch(/grande demais/);
    expect(validateLabel("fat32", "  ")).toEqual({ value: null, error: null });
  });
  it("demais: mantém caixa e limita bytes UTF-8", () => {
    expect(validateLabel("exfat", "Backup Fotos")).toEqual({ value: "Backup Fotos", error: null });
    expect(validateLabel("exfat", "ççaaaaaaaaaaaaa").error).toMatch(/grande demais/);
    expect(validateLabel("ext4", "dezesseis-letras").error).toBeNull();
    expect(validateLabel("ext4", "dezessete-letras!").error).toMatch(/grande demais/);
  });
});
