import { render, screen, fireEvent } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { RepairTab } from "./RepairTab";
import { useApp } from "../../lib/store";
import { view } from "../../test/fixtures";

const start = vi.fn((..._a: unknown[]) => Promise.resolve(1));
vi.mock("../../lib/api", () => ({ startOperation: (...a: unknown[]) => start(...a) }));
beforeEach(() => {
  start.mockClear();
  useApp.setState({ ops: {}, finished: {} });
});

describe("aba Reparar", () => {
  it("verificar inicia sem pedir a série", () => {
    render(<RepairTab view={view()} />);
    fireEvent.click(screen.getByRole("button", { name: /Verificar/ }));
    expect(start).toHaveBeenCalledWith({ kind: "repair", device: "/dev/sdb", check: true }, "ABC");
  });

  it("reparar pede a série antes", () => {
    render(<RepairTab view={view()} />);
    fireEvent.click(screen.getByRole("button", { name: /^Reparar/ }));
    expect(start).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog")).toBeTruthy();
    expect(screen.getByText(/FSCK0000.REC/)).toBeTruthy();
  });

  it("mostra o resultado por partição", () => {
    useApp.setState({
      finished: {
        "/dev/sdb": {
          repair: {
            id: 1, device: "/dev/sdb", kind: "repair", ok: true, error: null, holders: [],
            result: { report: { check_only: true, table_warning: null, partitions: [
              { path: "/dev/sdb1", fstype: "vfat", status: "errors_remain", log: ["Dirty bit is set."], note: null },
            ] } },
          },
        },
      },
    });
    render(<RepairTab view={view()} />);
    expect(screen.getByText("/dev/sdb1")).toBeTruthy();
    expect(screen.getByText(/tem problemas/)).toBeTruthy();
    expect(screen.getByText("Dirty bit is set.")).toBeTruthy();
  });
});
