import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Sidebar } from "./Sidebar";
import { view } from "../../test/fixtures";

describe("Sidebar", () => {
  it("mostra estado vazio", () => {
    render(<Sidebar devices={[]} selected={null} onSelect={() => {}} />);
    expect(screen.getByText(/Conecte um pendrive/)).toBeTruthy();
  });

  it("marca disco do sistema, USB 2 limitado e veredito falso", () => {
    render(
      <Sidebar
        devices={[view({ last_fake_verdict: "fake" }), view({ path: "/dev/nvme0n1", model: "SSD", is_system: true })]}
        selected="/dev/sdb"
        onSelect={() => {}}
      />,
    );
    expect(screen.getByLabelText("disco do sistema")).toBeTruthy();
    expect(screen.getAllByText("USB 2.0 ⚠").length).toBeGreaterThan(0);
    expect(screen.getByText("FALSO")).toBeTruthy();
  });

  it("sem USB, mostra a dica do WSL", () => {
    const nvme = view({ path: "/dev/nvme0n1", is_system: true });
    nvme.device.transport = "nvme";
    nvme.device.usb = null;
    render(<Sidebar devices={[nvme]} selected={null} onSelect={() => {}} hint="usbipd attach" />);
    expect(screen.getByText(/Conecte um pendrive/)).toBeTruthy();
    expect(screen.getByText("usbipd attach")).toBeTruthy();
  });

  it("seleciona ao clicar", () => {
    const onSelect = vi.fn();
    render(<Sidebar devices={[view()]} selected={null} onSelect={onSelect} />);
    fireEvent.click(screen.getByText("Pen"));
    expect(onSelect).toHaveBeenCalledWith("/dev/sdb");
  });
});
