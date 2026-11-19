import { render, screen, fireEvent } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { ThemeSwitch } from "./ThemeSwitch";

beforeEach(() => {
  localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
});

describe("ThemeSwitch", () => {
  it("marca a opção e aplica o tema ao clicar", () => {
    render(<ThemeSwitch />);
    expect(screen.getByRole("radio", { name: "Sistema" }).getAttribute("aria-checked")).toBe("true");
    fireEvent.click(screen.getByRole("radio", { name: "Escuro" }));
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(screen.getByRole("radio", { name: "Escuro" }).getAttribute("aria-checked")).toBe("true");
  });
});
