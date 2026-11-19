import { useState } from "react";
import { startOperation } from "../../lib/api";
import { useApp } from "../../lib/store";
import type { DeviceView } from "../../lib/bindings/DeviceView";
import type { OpRequest } from "../../lib/bindings/OpRequest";

/** Inicia operações do disco; destrutivas passam antes pela confirmação. */
export function useOperation(view: DeviceView) {
  const path = view.device.path;
  const running = useApp((s) => s.ops[path]);
  const finished = useApp((s) => s.finished[path]);
  const opStarted = useApp((s) => s.opStarted);
  const [pending, setPending] = useState<{ req: OpRequest; title: string; lines: string[] } | null>(null);
  const [error, setError] = useState<string | null>(null);

  const launch = (req: OpRequest) => {
    setError(null);
    startOperation(req, view.confirmation_token)
      .then((id) => opStarted(path, id, req.kind))
      .catch((e) => setError(String(e)));
  };

  const run = (req: OpRequest, confirm?: { title: string; lines: string[] }) => {
    if (confirm) setPending({ req, ...confirm });
    else launch(req);
  };

  return {
    run,
    running,
    error,
    last: (kind: string) => finished?.[kind],
    confirm: pending && {
      title: pending.title,
      lines: pending.lines,
      onConfirm: () => {
        launch(pending.req);
        setPending(null);
      },
      onClose: () => setPending(null),
    },
  };
}
