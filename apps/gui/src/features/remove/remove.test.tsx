import { render, screen, fireEvent } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useApp } from "../../lib/store";
import { view } from "../../test/fixtures";
import { RemoveButton } from "./RemoveButton";
import { InUseCard } from "./InUseCard";
import { Notice } from "./Notice";

vi.mock("../../lib/api", () => ({ startOperation: vi.fn(() => Promise.resolve(1)) }));

beforeEach(() => useApp.setState({ ops: {}, finished: {}, notice: null, devices: [view()] }));

describe("remover com segurança", () => {
  it("bloqueado durante uma operação no disco", () => {
    useApp.setState({
      ops: { "/dev/sdb": { id: 1, kind: "fake_check", startedAt: 0, phaseStartedAt: 0, progress: null, cancelling: false } },
    });
    render(<RemoveButton view={view()} />);
    const b = screen.getByRole("button", { name: /Remover com segurança/ }) as HTMLButtonElement;
    expect(b.disabled).toBe(true);
    expect(b.title).toMatch(/operação em andamento/);
  });

  it("cartão em uso lista os processos, explica e tenta de novo", () => {
    const onRetry = vi.fn();
    render(<InUseCard error="/media/x está em uso" holders={[{ pid: 7, command: "dolphin", path: "/media/x" }]} onRetry={onRetry} />);
    expect(screen.getByText("dolphin")).toBeTruthy();
    expect(screen.getByText(/gerenciador de arquivos/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Tentar de novo" }));
    expect(onRetry).toHaveBeenCalled();
  });

  it("sem processos visíveis mostra a mensagem da CLI", () => {
    render(<InUseCard error={"está em uso por um processo que não consigo ver"} holders={[]} onRetry={() => {}} />);
    expect(screen.getByText(/não consigo ver/)).toBeTruthy();
  });

  it("sucesso gera o aviso 'pode tirar' com o modelo, mesmo depois de o disco sumir", () => {
    useApp.getState().opFinished({
      id: 1, device: "/dev/sdb", kind: "eject", ok: true,
      result: { device: "/dev/sdb", powered_off: true, note: null }, error: null, holders: [],
    });
    useApp.setState({ devices: [] });
    render(<Notice />);
    expect(screen.getByText(/Pode tirar o Pen \(sdb\)/)).toBeTruthy();
    fireEvent.click(screen.getByRole("status"));
    expect(useApp.getState().notice).toBeNull();
  });

  it("só desmontou: avisa para esperar o LED", () => {
    useApp.getState().opFinished({
      id: 1, device: "/dev/sdb", kind: "eject", ok: true,
      result: { device: "/dev/sdb", powered_off: false, note: "não suportado" }, error: null, holders: [],
    });
    expect(useApp.getState().notice?.text).toMatch(/LED parar de piscar/);
    expect(useApp.getState().notice?.tone).toBe("warn");
  });

  it("falha não gera aviso de sucesso", () => {
    useApp.getState().opFinished({ id: 1, device: "/dev/sdb", kind: "eject", ok: false, result: null, error: "em uso", holders: [] });
    expect(useApp.getState().notice).toBeNull();
  });
});
