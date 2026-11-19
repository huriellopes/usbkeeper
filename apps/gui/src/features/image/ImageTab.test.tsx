import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ImageTab } from "./ImageTab";
import { useApp } from "../../lib/store";
import { view } from "../../test/fixtures";

const start = vi.fn((..._a: unknown[]) => Promise.resolve(1));
const meta = vi.fn((..._a: unknown[]) => Promise.resolve({ model: "Kingston DataTraveler", size_bytes: 64_000_000_000, created_at: "2026-11-19 10:00 UTC" }));
vi.mock("../../lib/api", () => ({
  startOperation: (...a: unknown[]) => start(...a),
  imageMeta: (...a: unknown[]) => meta(...a),
}));
const save = vi.fn(() => Promise.resolve("/home/ana/pen.img.zst"));
const open = vi.fn(() => Promise.resolve("/home/ana/arquivo"));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: () => save(), open: () => open() }));

beforeEach(() => {
  start.mockClear();
  useApp.setState({ ops: {}, finished: {} });
});

describe("aba Backup e ISO", () => {
  it("criar imagem escolhe o arquivo e não pede a série", async () => {
    render(<ImageTab view={view()} />);
    fireEvent.click(screen.getByRole("button", { name: /Criar imagem/ }));
    await waitFor(() => expect(start).toHaveBeenCalledWith({ kind: "image_create", device: "/dev/sdb", output: "/home/ana/pen.img.zst" }, "ABC"));
  });

  it("restaurar mostra de onde é a imagem e pede a série", async () => {
    render(<ImageTab view={view()} />);
    fireEvent.click(screen.getByRole("button", { name: /Restaurar imagem/ }));
    await waitFor(() => expect(screen.getByRole("dialog")).toBeTruthy());
    expect(screen.getByText(/Kingston DataTraveler/)).toBeTruthy();
    expect(start).not.toHaveBeenCalled();
  });

  it("gravar ISO leva o hash informado e pede a série", async () => {
    render(<ImageTab view={view()} />);
    fireEvent.change(screen.getByLabelText(/SHA-256 da ISO/), { target: { value: "ab".repeat(32) } });
    fireEvent.click(screen.getByRole("button", { name: /Gravar ISO/ }));
    await waitFor(() => expect(screen.getByRole("dialog")).toBeTruthy());
    fireEvent.change(screen.getByLabelText(/Digite ABC/), { target: { value: "ABC" } });
    fireEvent.click(screen.getByRole("button", { name: "Gravar" }));
    expect(start).toHaveBeenCalledWith({ kind: "burn_iso", device: "/dev/sdb", iso: "/home/ana/arquivo", sha256: "ab".repeat(32) }, "ABC");
  });
});
