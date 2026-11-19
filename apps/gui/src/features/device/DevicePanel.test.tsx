import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { DevicePanel } from "./DevicePanel";
import { view } from "../../test/fixtures";

vi.mock("../../lib/api", () => ({
  deviceHolders: () => Promise.resolve([]),
  cancelOperation: () => Promise.resolve(),
  startOperation: () => Promise.resolve(1),
  historyEvents: () => Promise.resolve([]),
  saveReport: () => Promise.resolve(),
}));

const tab = (name: string) => screen.getByRole("tab", { name }) as HTMLButtonElement;

describe("DevicePanel", () => {
  it("disco do sistema: sem Verificar/Formatar e sem Remover", () => {
    render(<DevicePanel view={view({ is_system: true, refusals: [{ kind: "system_disk", mountpoint: "/" }] })} />);
    expect(tab("Verificar").disabled).toBe(true);
    expect(tab("Formatar").disabled).toBe(true);
    expect(tab("Reparar").disabled).toBe(true);
    expect(tab("Saúde").disabled).toBe(false);
    expect(screen.queryByRole("button", { name: /Remover com segurança/ })).toBeNull();
  });

  it("pendrive: todas as abas e Remover com segurança disponíveis", () => {
    render(<DevicePanel view={view()} />);
    for (const name of ["Visão geral", "Verificar", "Saúde", "Velocidade", "Formatar", "Reparar", "Histórico"]) {
      expect(tab(name).disabled).toBe(false);
    }
    expect(screen.getByRole("button", { name: /Remover com segurança/ })).toBeTruthy();
  });
});
