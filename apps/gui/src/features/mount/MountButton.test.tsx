import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { MountButton } from "./MountButton";
import { view } from "../../test/fixtures";

const mountDevice = vi.fn((..._a: unknown[]) => Promise.resolve(["/media/ana/PEN"]));
const openFolder = vi.fn((..._a: unknown[]) => Promise.resolve());
vi.mock("../../lib/api", () => ({
  mountDevice: (...a: unknown[]) => mountDevice(...a),
  openFolder: (...a: unknown[]) => openFolder(...a),
}));
beforeEach(() => { mountDevice.mockClear(); openFolder.mockClear(); });

const part = (mountpoints: string[], fstype: string | null = "vfat") => ({
  name: "sdb1", path: "/dev/sdb1", size_bytes: 1, fstype, label: "PEN", mountpoints,
});

describe("montar / abrir pasta", () => {
  it("desmontado com FAT32: Montar e, depois, Abrir pasta", async () => {
    const v = view();
    v.device.partitions = [part([])];
    render(<MountButton view={v} />);
    fireEvent.click(screen.getByRole("button", { name: "Montar" }));
    expect(mountDevice).toHaveBeenCalledWith("/dev/sdb");
    await waitFor(() => expect(screen.getByRole("button", { name: "Abrir pasta" })).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: "Abrir pasta" }));
    expect(openFolder).toHaveBeenCalledWith("/dev/sdb", "/media/ana/PEN");
  });

  it("já montado: só Abrir pasta", () => {
    const v = view();
    v.device.partitions = [part(["/media/ana/PEN"])];
    render(<MountButton view={v} />);
    expect(screen.queryByRole("button", { name: "Montar" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Abrir pasta" }));
    expect(openFolder).toHaveBeenCalledWith("/dev/sdb", "/media/ana/PEN");
  });

  it("Windows: a letra (E:\\) abre no Explorer; pasta montada não", () => {
    const v = view({ path: "\\\\.\\PhysicalDrive1" });
    v.device.partitions = [part(["E:\\pasta", "E:\\"], "vfat")];
    render(<MountButton view={v} />);
    fireEvent.click(screen.getByRole("button", { name: "Abrir pasta" }));
    expect(openFolder).toHaveBeenCalledWith("\\\\.\\PhysicalDrive1", "E:\\");
  });

  it("sem sistema de arquivos montável: nada a mostrar", () => {
    const v = view();
    v.device.partitions = [part([], "crypto_LUKS")];
    const { container } = render(<MountButton view={v} />);
    expect(container.textContent).toBe("");
  });
});
