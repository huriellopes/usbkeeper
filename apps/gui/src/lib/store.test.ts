import { describe, expect, it } from "vitest";
import { useApp } from "./store";
import { view } from "../test/fixtures";

describe("store.setDevices", () => {
  it("lista igual não troca o estado (sem redesenho)", () => {
    useApp.getState().setDevices([view()]);
    const before = useApp.getState().devices;
    useApp.getState().setDevices([view()]);
    expect(useApp.getState().devices).toBe(before);
    useApp.getState().setDevices([view({ last_fake_verdict: "fake" })]);
    expect(useApp.getState().devices).not.toBe(before);
  });

  it("seleciona o primeiro disco que não é do sistema", () => {
    useApp.setState({ selected: null });
    useApp.getState().setDevices([view({ path: "/dev/nvme0n1", is_system: true }), view({ path: "/dev/sdc" })]);
    expect(useApp.getState().selected).toBe("/dev/sdc");
  });
});
