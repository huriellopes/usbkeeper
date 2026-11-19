import { beforeEach, describe, expect, it } from "vitest";
import { applyTheme, getTheme, setTheme } from "./theme";

beforeEach(() => {
  localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
});

describe("tema", () => {
  it("padrão é seguir o sistema", () => {
    expect(getTheme()).toBe("system");
  });

  it("escuro e claro gravam e aplicam data-theme; sistema remove", () => {
    setTheme("dark");
    expect(getTheme()).toBe("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
    setTheme("light");
    expect(document.documentElement.dataset.theme).toBe("light");
    setTheme("system");
    expect(getTheme()).toBe("system");
    expect(document.documentElement.hasAttribute("data-theme")).toBe(false);
  });

  it("valor inválido no armazenamento volta para sistema", () => {
    localStorage.setItem("usbkeeper.theme", "roxo");
    expect(getTheme()).toBe("system");
    applyTheme(getTheme());
    expect(document.documentElement.hasAttribute("data-theme")).toBe(false);
  });
});
