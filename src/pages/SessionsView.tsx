import { useState } from "react";
import { useSessionsStore, type ToolCallItem } from "../store/sessions";
import { NewSessionForm } from "../components/NewSessionForm";
import { Wrench, CheckCircle2, XCircle, Loader2 } from "lucide-react";

function ToolCallBlock({ call }: { call: ToolCallItem }) {
  const pending = !call.result && !call.denied;
  const denied = call.denied || (call.result?.isError && call.result.output.includes("Refusé"));
  return (
    <div
      className={`mb-3 rounded border px-3 py-2 text-xs ${
        denied
          ? "border-red-500/40 bg-red-500/5"
          : call.result?.isError
            ? "border-yellow-500/40 bg-yellow-500/5"
            : "border-accent/40 bg-accent/5"
      }`}
    >
      <div className="mb-1 flex items-center gap-2 font-semibold">
        {pending ? (
          <Loader2 size={14} className="animate-spin text-accent" />
        ) : denied ? (
          <XCircle size={14} className="text-red-400" />
        ) : (
          <CheckCircle2 size={14} className="text-green-400" />
        )}
        <Wrench size={14} className="text-muted" />
        <span className="font-mono">{call.tool}</span>
        {pending && <span className="text-muted">— en attente d'approbation…</span>}
      </div>
      <details open={!!call.result}>
        <summary className="cursor-pointer text-muted">Arguments</summary>
        <pre className="mt-1 whitespace-pre-wrap font-mono text-xs">
          {JSON.stringify(call.arguments, null, 2)}
        </pre>
      </details>
      {call.result && (
        <div className="mt-2 border-t border-border/40 pt-2">
          <div className="mb-1 text-[10px] uppercase tracking-wider text-muted">
            Résultat {call.result.isError ? "(erreur)" : ""}
          </div>
          <pre className="max-h-60 overflow-y-auto whitespace-pre-wrap font-mono text-xs">
            {call.result.output}
          </pre>
        </div>
      )}
    </div>
  );
}

export function SessionsView() {
  const {
    sessions,
    activeSessionId,
    messages,
    toolCalls,
    streaming,
    errors,
    creating,
    createSession,
    cancelCreating,
    send,
    cancel,
  } = useSessionsStore();

  const [input, setInput] = useState("");

  const active = sessions.find((s) => s.id === activeSessionId);
  const msgs = active ? messages[active.id] ?? [] : [];
  const calls = active ? toolCalls[active.id] ?? [] : [];
  const isStreaming = active ? streaming[active.id] ?? false : false;
  const error = active ? errors[active.id] ?? null : null;

  if (creating) {
    return (
      <NewSessionForm
        onCreate={(p) => createSession(p)}
        onCancel={cancelCreating}
      />
    );
  }

  if (!active) {
    return (
      <div className="flex flex-1 items-center justify-center px-6 text-center text-sm text-muted">
        Bienvenue dans Cyonima. Cliquez sur <span className="px-1 font-semibold text-fg">+ Nouvelle session</span> dans la barre latérale.
      </div>
    );
  }

  const submit = () => {
    const text = input.trim();
    if (!text || isStreaming) return;
    setInput("");
    void send(active.id, text);
  };

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      <header className="border-b border-border px-4 py-2 text-xs text-muted">
        {active.modelId} • {active.providerId} • {active.workspace}
      </header>

      <div className="flex-1 overflow-y-auto px-4 py-4">
        {msgs.length === 0 && calls.length === 0 && (
          <p className="text-sm text-muted">Posez votre première question…</p>
        )}
        {msgs.map((m, i) => (
          <div key={i} className="mb-4 text-sm">
            <div className="mb-1 text-xs font-semibold text-muted">{m.role}</div>
            <div className="whitespace-pre-wrap">{m.content}</div>
          </div>
        ))}
        {/* Tool calls en cours/derniers — affichés après le dernier message assistant,
            comme OkTok y insère son "navigation". */}
        {calls.map((c) => (
          <ToolCallBlock key={c.callId} call={c} />
        ))}
        {error && (
          <div className="mb-4 rounded border border-red-500/40 bg-red-500/10 px-3 py-2 text-xs text-red-300">
            {error}
          </div>
        )}
      </div>

      <div className="border-t border-border p-3">
        <div className="flex items-end gap-2">
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                submit();
              }
            }}
            placeholder="Écrivez un message… (Entrée pour envoyer, Maj+Entrée = saut de ligne)"
            rows={2}
            className="flex-1 resize-none rounded border border-border bg-bg px-3 py-2 text-sm focus:border-accent focus:outline-none"
          />
          {isStreaming ? (
            <button
              onClick={() => void cancel(active.id)}
              className="rounded border border-red-500/40 px-3 py-2 text-xs text-red-300 hover:bg-red-500/10"
            >
              Stop
            </button>
          ) : (
            <button
              onClick={submit}
              disabled={!input.trim()}
              className="rounded bg-accent px-4 py-2 text-xs text-white disabled:opacity-50"
            >
              Envoyer
            </button>
          )}
        </div>
      </div>
    </div>
  );
}