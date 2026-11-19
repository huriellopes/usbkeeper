import { render, screen, fireEvent } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SelfTestPanel } from "./SelfTestPanel";
import { useApp } from "../../lib/store";
import { view } from "../../test/fixtures";

const start = vi.fn((..._a: unknown[]) => Promise.resolve(1));
vi.mock("../../lib/api", () => ({ startOperation: (...a: unknown[]) => start(...a) }));
beforeEach(() => {
  start.mockClear();
  useApp.setState({ ops: {}, finished: {} });
});

describe("autoteste", () => {
  it("curto e longo disparam sem pedir a série", () => {
    render(<SelfTestPanel view={view()} />);
    fireEvent.click(screen.getByRole("button", { name: /Autoteste curto/ }));
    expect(start).toHaveBeenCalledWith({ kind: "selftest", device: "/dev/sdb", long: false }, "ABC");
    fireEvent.click(screen.getByRole("button", { name: /Autoteste longo/ }));
    expect(start).toHaveBeenCalledWith({ kind: "selftest", device: "/dev/sdb", long: true }, "ABC");
  });

  it("mostra o resultado e os últimos autotestes", () => {
    useApp.setState({
      finished: {
        "/dev/sdb": {
          selftest: {
            id: 1, device: "/dev/sdb", kind: "selftest", ok: true, error: null, holders: [],
            result: {
              verdict: "failed",
              result: { kind: "Extended offline", status: "Completed: read failure", passed: false, hours: 20000 },
              state: { running: false, remaining_percent: null, polling_minutes: 110, log: [
                { kind: "Extended offline", status: "Completed: read failure", passed: false, hours: 20000 },
              ] },
            },
          },
        },
      },
    });
    render(<SelfTestPanel view={view()} />);
    expect(screen.getByText(/REPROVADO/)).toBeTruthy();
    expect(screen.getAllByText(/read failure/).length).toBeGreaterThan(0);
  });
});
