import { render, screen, fireEvent } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { WipeTab } from "./WipeTab";
import { useApp } from "../../lib/store";
import { view } from "../../test/fixtures";

const start = vi.fn((..._a: unknown[]) => Promise.resolve(1));
vi.mock("../../lib/api", () => ({ startOperation: (...a: unknown[]) => start(...a) }));
beforeEach(() => {
  start.mockClear();
  useApp.setState({ ops: {}, finished: {} });
});

describe("aba Apagar", () => {
  it("avisa sobre flash e exige a série antes de apagar", () => {
    render(<WipeTab view={view()} onFormat={() => {}} />);
    expect(screen.getByText(/áreas de reserva/)).toBeTruthy();
    fireEvent.click(screen.getByLabelText(/Verificar no fim/));
    fireEvent.click(screen.getByRole("button", { name: /Apagar tudo/ }));
    expect(start).not.toHaveBeenCalled();
    const input = screen.getByLabelText(/Digite ABC/);
    fireEvent.change(input, { target: { value: "ABC" } });
    fireEvent.click(screen.getByRole("button", { name: "Apagar" }));
    expect(start).toHaveBeenCalledWith(
      { kind: "wipe", device: "/dev/sdb", random: false, passes: 1, verify: true, discard: false },
      "ABC",
    );
  });

  it("descartar desliga as opções de sobrescrita", () => {
    render(<WipeTab view={view()} onFormat={() => {}} />);
    fireEvent.click(screen.getByRole("radio", { name: /Descartar/ }));
    expect((screen.getByLabelText(/Passadas/) as HTMLSelectElement).disabled).toBe(true);
  });

  it("depois de apagar oferece formatar", () => {
    const onFormat = vi.fn();
    useApp.setState({
      finished: { "/dev/sdb": { wipe: { id: 1, device: "/dev/sdb", kind: "wipe", ok: true, error: null, holders: [],
        result: { report: { bytes: 1, passes: 1, pattern: "zeros", verified: true, mismatched_bytes: 0, seconds: 1 } } } } },
    });
    render(<WipeTab view={view()} onFormat={onFormat} />);
    expect(screen.getByText(/verificado/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Formatar agora" }));
    expect(onFormat).toHaveBeenCalled();
  });
});
