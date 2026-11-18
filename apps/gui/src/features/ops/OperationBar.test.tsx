import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { OperationBar, eta } from "./OperationBar";
import type { RunningOp } from "../../lib/store";

const op = (over: Partial<RunningOp> = {}): RunningOp => ({
  id: 1, kind: "fake_check", startedAt: 0, phaseStartedAt: 0, cancelling: false,
  progress: { phase: "write", label: "gravando", done: 50_000_000, total: 100_000_000, bytes: true },
  ...over,
});

describe("OperationBar", () => {
  it("mostra etapa, porcentagem e ETA", () => {
    render(<OperationBar op={op()} onCancel={() => {}} now={5000} />);
    expect(screen.getByText("gravando")).toBeTruthy();
    expect(screen.getByText("50%")).toBeTruthy();
    expect(screen.getByText(/10.0 MB\/s · restam ~5s/)).toBeTruthy();
  });

  it("sem progresso ainda: aguardando autorização", () => {
    render(<OperationBar op={op({ progress: null })} onCancel={() => {}} />);
    expect(screen.getByText(/aguardando autorização/)).toBeTruthy();
  });

  it("cancelar chama o callback e desabilita ao cancelar", () => {
    const onCancel = vi.fn();
    const { rerender } = render(<OperationBar op={op()} onCancel={onCancel} now={5000} />);
    fireEvent.click(screen.getByText("Cancelar"));
    expect(onCancel).toHaveBeenCalled();
    rerender(<OperationBar op={op({ cancelling: true })} onCancel={onCancel} now={5000} />);
    expect((screen.getByText("Cancelando…") as HTMLButtonElement).disabled).toBe(true);
  });

  it("etapas que não contam bytes não têm ETA", () => {
    expect(eta(op({ progress: { phase: "probe", label: "x", done: 5, total: 10, bytes: false } }), 5000)).toBeNull();
  });
});
