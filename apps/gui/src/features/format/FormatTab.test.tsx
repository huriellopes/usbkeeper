import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { FormatTab } from "./FormatTab";
import { view } from "../../test/fixtures";
import { useApp } from "../../lib/store";

describe("FormatTab", () => {
  it("sem ext4 na plataforma (Windows): não oferece ext4", () => {
    useApp.setState({ platform: { ext4: false, trim: false } });
    render(<FormatTab view={view()} onSubmit={() => {}} busy={false} />);
    const opts = [...(screen.getByLabelText("Sistema de arquivos") as HTMLSelectElement).options].map((o) => o.value);
    expect(opts).not.toContain("ext4");
    expect(opts).toContain("exfat");
    useApp.setState({ platform: { ext4: true, trim: true } });
  });

  it("preenche o limite sugerido e força MBR", () => {
    render(<FormatTab view={view()} initial={{ maxBytes: 7_000_000_000 }} onSubmit={() => {}} busy={false} />);
    expect((screen.getByLabelText(/Usar só os primeiros/) as HTMLInputElement).value).toBe("7000");
    expect((screen.getByLabelText("Tabela") as HTMLSelectElement).disabled).toBe(true);
  });

  it("aceita sistema de arquivos e tabela iniciais (assistente)", () => {
    render(<FormatTab view={view()} initial={{ filesystem: "ntfs", table: "gpt" }} onSubmit={() => {}} busy={false} />);
    expect((screen.getByLabelText("Sistema de arquivos") as HTMLSelectElement).value).toBe("ntfs");
    expect((screen.getByLabelText("Tabela") as HTMLSelectElement).value).toBe("gpt");
  });

  it("bloqueia rótulo inválido e envia pedido válido", () => {
    const onSubmit = vi.fn();
    render(<FormatTab view={view()} onSubmit={onSubmit} busy={false} />);
    fireEvent.change(screen.getByLabelText("Sistema de arquivos"), { target: { value: "fat32" } });
    fireEvent.change(screen.getByLabelText("Rótulo"), { target: { value: "câmera" } });
    expect(screen.getByText(/ASCII/)).toBeTruthy();
    expect((screen.getByRole("button", { name: /Formatar/ }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(screen.getByLabelText("Rótulo"), { target: { value: "fotos" } });
    fireEvent.click(screen.getByRole("button", { name: /Formatar/ }));
    expect(onSubmit).toHaveBeenCalledWith({
      kind: "format", device: "/dev/sdb", filesystem: "fat32", table: null, label: "FOTOS", max_bytes: null,
    });
  });
});
