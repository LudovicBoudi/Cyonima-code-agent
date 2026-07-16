import { useState } from "react";
import { useSessionsStore } from "../store/sessions";
import { NewSessionForm } from "../components/NewSessionForm";

export function SessionsView() {
  const {
    sessions,
    activeSessionId,
    messages,
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
        {msgs.length === 0 && (
          <p className="text-sm text-muted">Posez votre première question…</p>
        )}
        {msgs.map((m, i) => (
          <div key={i} className="mb-4 text-sm">
            <div className="mb-1 text-xs font-semibold text-muted">{m.role}</div>
            <div className="whitespace-pre-wrap">{m.content}</div>
          </div>
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