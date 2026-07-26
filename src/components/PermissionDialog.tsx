import { ShieldCheck, ShieldAlert } from "lucide-react";
import { ipc, type PermissionRequestEvent } from "../lib/ipc";
import { useSessionsStore } from "../store/sessions";
import { usePermissionsStore } from "../store/permissions";

export function PermissionDialog() {
  const queue = usePermissionsStore((s) => s.queue);
  const markDecided = usePermissionsStore((s) => s.markDecided);
  const remove = usePermissionsStore((s) => s.remove);
  const setToolResult = useSessionsStore((s) => s.setToolResult);

  const respond = async (req: PermissionRequestEvent, decision: "allow" | "deny") => {
    markDecided(req.requestId);
    try {
      await ipc.permissionRespond({ requestId: req.requestId, decision });
      if (decision === "deny") {
        setToolResult(req.sessionId, req.requestId, "Refusé par l'utilisateur", true);
      }
    } catch (e) {
      console.error("permission_respond error", e);
    }
    setTimeout(() => remove(req.requestId), 50);
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
