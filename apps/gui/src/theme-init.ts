// Importado antes de tudo no main.tsx: aplica o tema escolhido antes do
// primeiro desenho, sem script inline (a CSP continua sem 'unsafe-inline').
import { applyTheme, getTheme } from "./lib/theme";

applyTheme(getTheme());
