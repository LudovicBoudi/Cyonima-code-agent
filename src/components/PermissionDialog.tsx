import { useEffect, useState } from "react";
import { ShieldCheck, ShieldAlert } from "lucide-react";
import { ipc, onPermissionRequest, type PermissionRequestEvent } from "../lib/ipc";
import { useSessionsStore } from "../store/sessions";

interface PendingDialog {
  request: PermissionRequestEvent;
  /// Approuvé/dénié localement en attendant la réponse du backend (sinon
  /// l'utilisateur pourrait cliquer deux fois).
  decided: boolean;
}

export function PermissionDialog() {
  const [queue, setQueue] = useState<PendingDialog[]>([]);
  const addToolCall = useSessionsStore((s) => s.addToolCall);
  const setToolResult = useSessionsStore((s) => s.setToolResult);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    (async () => {
      unlisten = await onPermissionRequest((req) => {
        setQueue((q) => [...q, { request: req, decided: false }]);
        // On ajoute aussi un tool call en attente dans le store pour que
        // l'utilisateur voie le "pending" inline dans le chat.
        addToolCall(req.sessionId, {
          callId: req.requestId,
          tool: req.tool,
          arguments: req.arguments,
        });
      });
    })();
    return () => {
      if (unlisten) unlisten();
    };
  }, [addToolCall]);

  const respond = async (req: PermissionRequestEvent, decision: "allow" | "deny") => {
    setQueue((q) => q.map((p) => (p.request.requestId === req.requestId ? { ...p, decided: true } : p)));
    try {
      await ipc.permissionRespond({ requestId: req.requestId, decision });
      if (decision === "deny") {
        setToolResult(req.sessionId, req.requestId, "Refusé par l'utilisateur", true);
      }
    } catch (e) {
      console.error("permission_respond error", e);
    }
    // On retire de la file après l'animation (50ms suffit pour le rendu).
    setTimeout(() => {
      setQueue((q) => q.filter((p) => p.request.requestId !== req.requestId));
    }, 50);
  };

  if (queue.length === 0) return null;
  const current = queue[0];

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="w-full max-w-md rounded border border-border bg-bg shadow-xl">
        <div className="flex items-center gap-2 border-b border-border px-4 py-3 text-sm font-semibold">
          {current.request.tool === "bash" ? (
            <ShieldAlert size={16} className="text-yellow-400" />
          ) : (
            <ShieldCheck size={16} className="text-accent" />
          )}
          Approbation requise — {current.request.tool}
        </div>
        <div className="px-4 py-3 text-sm">
          <p className="mb-2 text-muted">
            L'agent veut exécuter l'outil <code className="text-accent">{current.request.tool}</code>.
          </p>
          {current.request.preview && (
            <pre className="mb-3 whitespace-pre-wrap rounded border border-border p-2 font-mono text-xs">
              {current.request.preview}
            </pre>
          )}
          <details className="mb-3">
            <summary className="cursor-pointer text-xs text-muted">Arguments</summary>
            <pre className="mt-2 whitespace-pre-wrap rounded border border-border bg-bg p-2 font-mono text-xs">
              {JSON.stringify(current.request.arguments, null, 2)}
            </pre>
          </details>
        </div>
        <div className="flex justify-end gap-2 border-t border-border px-4 py-3">
          <button
            onClick={() => respond(current.request, "deny")}
            disabled={current.decided}
            className="rounded border border-border px-3 py-1.5 text-xs text-muted hover:bg-border/40 disabled:opacity-50"
          >
            Refuser
          </button>
          <button
            onClick={() => respond(current.request, "allow")}
            disabled={current.decided}
            className="rounded bg-accent px-4 py-1.5 text-xs text-white disabled:opacity-50"
          >
            Autoriser
          </button>
        </div>
      </div>
    </div>
  );
}