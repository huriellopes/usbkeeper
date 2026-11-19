import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { FormatTab } from "./FormatTab";
import { view } from "../../test/fixtures";

describe("FormatTab", () => {
  it("preenche o limite sugerido e força MBR", () => {
    render(<FormatTab view={view()} initialMaxBytes={7_000_000_000} onSubmit={() => {}} busy={false} />);
    expect((screen.getByLabelText(/Usar só os primeiros/) as HTMLInputElement).value).toBe("7000");
    expect((screen.getByLabelText("Tabela") as HTMLSelectElement).disabled).toBe(true);
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
