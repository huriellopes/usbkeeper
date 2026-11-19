// Tema da janela: "system" segue o sistema; "light"/"dark" fixam.
// Guardado em localStorage (preferência local desta máquina).

export type ThemePref = "system" | "light" | "dark";
const KEY = "usbkeeper.theme";

export function getTheme(): ThemePref {
  try {
    const v = localStorage.getItem(KEY);
    return v === "light" || v === "dark" ? v : "system";
  } catch {
    return "system";
  }
}

export function applyTheme(t: ThemePref) {
  const root = document.documentElement;
  if (t === "system") root.removeAttribute("data-theme");
  else root.dataset.theme = t;
}

export function setTheme(t: ThemePref) {
  try {
    if (t === "system") localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, t);
  } catch {
    // Sem armazenamento: vale só nesta sessão.
  }
  applyTheme(t);
}
