import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { HelpPanel } from "./HelpPanel";
import { HelpButton } from "./HelpButton";
import { useApp } from "../../lib/store";

const topics = [
  { id: "mbr-gpt", title: "Tabela de partições: MBR × GPT", summary: "O mapa do disco.", html: "<h1>MBR</h1><p>Mapa</p>" },
  { id: "remover", title: "Remover com segurança", summary: "Por que não arrancar.", html: "<h1>Remover</h1><p>Cache</p>" },
];

describe("ajuda", () => {
  it("o botão ? abre o tópico certo", () => {
    render(<HelpButton topic="remover" label="Remoção segura" />);
    fireEvent.click(screen.getByRole("button", { name: "Ajuda: Remoção segura" }));
    expect(useApp.getState().help).toEqual({ open: true, topic: "remover" });
  });

  it("mostra o tópico selecionado e a busca filtra o índice", () => {
    useApp.setState({ help: { open: true, topic: "remover" } });
    render(<HelpPanel topics={topics} />);
    expect(screen.getByText("Cache")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Buscar no guia"), { target: { value: "mapa" } });
    expect(screen.queryByRole("button", { name: /Remover com segurança/ })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /MBR × GPT/ }));
    expect(screen.getByText("Mapa")).toBeTruthy();
  });

  it("Voltar fecha a ajuda", () => {
    useApp.setState({ help: { open: true, topic: "remover" } });
    render(<HelpPanel topics={topics} />);
    fireEvent.click(screen.getByRole("button", { name: "← Voltar" }));
    expect(useApp.getState().help.open).toBe(false);
  });
});
