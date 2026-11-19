import { useState } from "react";
import { type ThemePref, getTheme, setTheme } from "../../lib/theme";

const OPTIONS: { id: ThemePref; label: string }[] = [
  { id: "system", label: "Sistema" },
  { id: "light", label: "Claro" },
  { id: "dark", label: "Escuro" },
];

export function ThemeSwitch() {
  const [pref, setPref] = useState<ThemePref>(getTheme());
  return (
    <div role="radiogroup" aria-label="Tema" className="flex rounded-lg p-0.5 text-xs" style={{ border: "1px solid var(--line)" }}>
      {OPTIONS.map((o) => (
        <button
          key={o.id}
          type="button"
          role="radio"
          aria-checked={pref === o.id}
          onClick={() => {
            setTheme(o.id);
            setPref(o.id);
          }}
          className="flex-1 rounded-md px-2 py-1"
          style={{ background: pref === o.id ? "var(--line)" : "transparent", fontWeight: pref === o.id ? 600 : 400 }}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}
