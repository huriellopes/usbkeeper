import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ConfirmDialog } from "./ConfirmDialog";

describe("ConfirmDialog", () => {
  it("só habilita com o token exato (aparando espaços)", () => {
    const onConfirm = vi.fn();
    render(
      <ConfirmDialog title="Formatar" token="ABC123" lines={["/dev/sdb1 apagado"]} confirmLabel="Formatar" onConfirm={onConfirm} onClose={() => {}} />,
    );
    const button = screen.getByRole("button", { name: "Formatar" }) as HTMLButtonElement;
    const input = screen.getByLabelText(/Digite ABC123/);
    expect(button.disabled).toBe(true);
    fireEvent.change(input, { target: { value: "abc123" } });
    expect(button.disabled).toBe(true);
    fireEvent.change(input, { target: { value: " ABC123 " } });
    expect(button.disabled).toBe(false);
    fireEvent.click(button);
    expect(onConfirm).toHaveBeenCalled();
    expect(screen.getByText(/\/dev\/sdb1 apagado/)).toBeTruthy();
  });
});
