import { useApp } from "../../lib/store";

/** "?" ao lado de uma decisão: abre o tópico do guia que a explica. */
export function HelpButton({ topic, label }: { topic: string; label: string }) {
  const openHelp = useApp((s) => s.openHelp);
  return (
    <button
      type="button"
      aria-label={`Ajuda: ${label}`}
      title={`Entenda: ${label}`}
      onClick={(e) => {
        e.preventDefault();
        e.stopPropagation();
        openHelp(topic);
      }}
      className="ml-1 inline-grid h-5 w-5 place-items-center rounded-full align-middle text-xs font-bold"
      style={{ border: "1px solid var(--line)", color: "var(--accent)" }}
    >
      ?
    </button>
  );
}
