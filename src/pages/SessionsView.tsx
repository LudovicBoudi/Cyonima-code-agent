import { useEffect, useState } from "react";
import { useSessionsStore, type ToolCallItem } from "../store/sessions";
import { NewSessionForm } from "../components/NewSessionForm";
import { ModelLoadingScreen } from "../components/ModelLoadingScreen";
import { Wrench, CheckCircle2, XCircle, Loader2, User, Bot, Brain } from "lucide-react";
import Markdown from "react-markdown";
import rehypeHighlight from "rehype-highlight";
import { DiffViewer } from "../components/DiffViewer";

const ROLE_META: Record<string, { label: string; icon: React.ReactNode }> = {
  user: { label: "Vous", icon: <User size={12} /> },
  assistant: { label: "Assistant", icon: <Bot size={12} /> },
  system: { label: "Système", icon: <Wrench size={12} /> },
};

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
          {call.tool === "edit_file" && !call.result.isError ? (
            <DiffViewer content={call.result.output} />
          ) : (
            <pre className="max-h-60 overflow-y-auto whitespace-pre-wrap font-mono text-xs">
              {call.result.output}
            </pre>
          )}
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
    thinking,
    modelLoading,
    modelLoadingProgress,
    creating,
    loaded,
    selectedModels,
    installedOllamaModels,
    restoreMessages,
    createSession,
    cancelCreating,
    send,
    cancel,
    setActive,
    setModelLoading,
    markModelReady,
    setSelectedModel,
    loadInstalledOllamaModels,
  } = useSessionsStore();

  const [input, setInput] = useState("");

  const active = sessions.find((s) => s.id === activeSessionId);
  const activeId = active?.id;
  const msgs = active ? messages[active.id] ?? [] : [];
  // On masque les messages `system` (AGENTS.md injecté pour le LLM) : ils
  // polluent la vue. Le LLM les reçoit toujours côté backend.
  const visibleMsgs = msgs.filter((m) => m.role !== "system");
  const calls = active ? toolCalls[active.id] ?? [] : [];
  const isStreaming = active ? streaming[active.id] ?? false : false;
  const error = active ? errors[active.id] ?? null : null;
  const activeThinking = active ? thinking[active.id] ?? "" : "";
  const isModelLoading = active ? modelLoading[active.id] ?? false : false;
  const loadingProgress = active ? modelLoadingProgress[active.id] ?? 0 : 0;
  const selectedModel = active ? selectedModels[active.id] ?? "" : "";
  const hasModels = installedOllamaModels.length > 0;

  useEffect(() => {
    if (!loaded || !activeId) return;
    const current = messages[activeId];
    if (current === undefined) {
      void restoreMessages(activeId);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeId, loaded]);

  // Rafraîchit la liste des modèles Ollama à l'ouverture de la vue.
  useEffect(() => {
    void loadInstalledOllamaModels();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Pré-sélectionne le premier modèle installé si la session n'en a pas encore.
  useEffect(() => {
    if (activeId && !selectedModel && installedOllamaModels.length > 0) {
      setSelectedModel(activeId, installedOllamaModels[0].name);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeId, installedOllamaModels, selectedModel]);

  if (creating) {
    return (
      <NewSessionForm
        onCreate={(p) => createSession(p)}
        onCancel={cancelCreating}
      />
    );
  }

  if (active && isModelLoading) {
    return (
      <ModelLoadingScreen
        modelId={active.modelId}
        progress={loadingProgress}
        onCancel={() => {
          setModelLoading(active.id, false);
          setActive(null);
        }}
        onSkip={() => {
          markModelReady(active.id);
        }}
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
    if (!text || isStreaming || !selectedModel) return;
    setInput("");
    void send(active.id, text);
  };

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      <header className="flex items-center gap-2 border-b border-border px-4 py-2 text-xs text-muted">
        <span className="text-muted">Modèle</span>
        {hasModels ? (
          <select
            value={selectedModel}
            onChange={(e) => setSelectedModel(active.id, e.target.value)}
            disabled={isStreaming}
            className="rounded border border-border bg-surface px-2 py-1 text-xs text-fg focus:border-accent focus:outline-none disabled:opacity-50"
          >
            {installedOllamaModels.map((m) => (
              <option key={m.name} value={m.name}>
                {m.name}
              </option>
            ))}
          </select>
        ) : (
          <span className="text-yellow-400">
            Aucun modèle installé — installez-en un via l'onglet Ollama
          </span>
        )}
        <span>•</span>
        <span>ollama</span>
        <span>•</span>
        <span className="truncate font-mono" title={active.workspace}>
          {active.workspace}
        </span>
      </header>

      <div className="flex-1 overflow-y-auto px-4 py-4">
        {/* Message de bienvenue court (remplace le dump AGENTS.md system). */}
        <div className="mb-4 text-sm">
          <div className="mb-1 flex items-center gap-1.5 text-xs font-semibold text-muted">
            <Bot size={12} />
            Assistant
          </div>
          <div className="text-muted">
            Bonjour ! Je suis prêt à vous aider sur ce projet. Posez votre question.
          </div>
        </div>
        {visibleMsgs.map((m, i) => {
          const meta = ROLE_META[m.role] ?? ROLE_META.user;
          const isLastAssistant = m.role === "assistant" && i === visibleMsgs.length - 1;
          return (
            <div key={i} className="mb-4 text-sm">
              <div className="mb-1 flex items-center gap-1.5 text-xs font-semibold text-muted">
                {meta.icon}
                {meta.label}
              </div>
              {isLastAssistant && activeThinking && (
                <details open className="mb-2">
                  <summary className="flex cursor-pointer items-center gap-1.5 text-xs text-muted hover:text-fg">
                    <Brain size={12} className="text-purple-400" />
                    Raisonnement du modèle
                  </summary>
                  <div className="mt-1 rounded border border-purple-500/20 bg-purple-500/5 px-3 py-2 text-xs text-muted whitespace-pre-wrap max-h-60 overflow-y-auto">
                    {activeThinking}
                  </div>
                </details>
              )}
              {m.role === "assistant" ? (
                <div className="prose prose-invert prose-sm max-w-none prose-pre:bg-surface prose-pre:border prose-pre:border-border prose-code:text-accent">
                  <Markdown rehypePlugins={[rehypeHighlight]}>
                    {m.content}
                  </Markdown>
                </div>
              ) : (
                <div className="whitespace-pre-wrap">{m.content}</div>
              )}
            </div>
          );
        })}
        {/* Affichage du thinking en temps réel */}
        {isStreaming && activeThinking && (
          <div className="mb-4 text-sm">
            <div className="mb-1 flex items-center gap-1.5 text-xs font-semibold text-muted">
              <Bot size={12} />
              Assistant
            </div>
            <details open className="mb-2">
              <summary className="flex cursor-pointer items-center gap-1.5 text-xs text-muted hover:text-fg">
                <Brain size={12} className="animate-pulse text-purple-400" />
                Raisonnement en cours...
              </summary>
              <div className="mt-1 rounded border border-purple-500/20 bg-purple-500/5 px-3 py-2 text-xs text-muted whitespace-pre-wrap max-h-60 overflow-y-auto">
                {activeThinking}
              </div>
            </details>
          </div>
        )}
        {/* Animation générique pendant le streaming sans thinking visible */}
        {isStreaming && !activeThinking && (visibleMsgs.length === 0 || visibleMsgs[visibleMsgs.length - 1]?.role !== "assistant") && (
          <div className="mb-4 flex items-center gap-2 text-xs text-muted">
            <Loader2 size={14} className="animate-spin text-accent" />
            <span className="animate-pulse">En train de générer…</span>
          </div>
        )}
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
            onInput={(e) => {
              const t = e.currentTarget;
              t.style.height = "auto";
              t.style.height = Math.min(t.scrollHeight, 160) + "px";
            }}
            placeholder="Écrivez un message… (Entrée pour envoyer, Maj+Entrée = saut de ligne)"
            rows={2}
            className="flex-1 resize-none rounded border border-border bg-surface px-3 py-2 text-sm focus:border-accent focus:outline-none max-h-40"
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
              disabled={!input.trim() || !selectedModel}
              title={!selectedModel ? "Sélectionnez un modèle" : undefined}
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
