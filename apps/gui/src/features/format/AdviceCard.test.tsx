import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AdviceCard } from "./AdviceCard";

const advise = vi.fn((..._args: unknown[]) =>
  Promise.resolve({
    filesystem: "fat32",
    table: "mbr",
    max_bytes: null,
    reasons: ["FAT32 é o formato que TVs leem."],
    warnings: ["Arquivos acima de 4 GB não cabem."],
  }),
);
vi.mock("../../lib/api", () => ({ advise: (...a: unknown[]) => advise(...a) }));

describe("assistente de formatação", () => {
  it("pergunta, mostra a recomendação explicada e aplica", async () => {
    const onApply = vi.fn();
    render(<AdviceCard device="/dev/sdb" onApply={onApply} />);
    const ver = screen.getByRole("button", { name: "Ver recomendação" }) as HTMLButtonElement;
    expect(ver.disabled).toBe(true);
    fireEvent.click(screen.getByRole("radio", { name: /TV, som, carro/ }));
    fireEvent.click(screen.getByRole("radio", { name: "Sim" }));
    fireEvent.click(ver);
    await waitFor(() => expect(screen.getByText(/FAT32 é o formato/)).toBeTruthy());
    expect(advise).toHaveBeenCalledWith("/dev/sdb", "appliance", "yes");
    expect(screen.getByText(/4 GB não cabem/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Usar esta configuração" }));
    expect(onApply).toHaveBeenCalledWith(expect.objectContaining({ filesystem: "fat32", table: "mbr" }));
  });
});
