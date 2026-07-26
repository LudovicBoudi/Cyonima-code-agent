import { useCallback, useEffect, useRef, useState } from "react";
import { useSessionsStore, type ToolCallItem } from "../store/sessions";
import { NewSessionForm } from "../components/NewSessionForm";
import { ModelLoadingScreen } from "../components/ModelLoadingScreen";
import {
  Wrench, CheckCircle2, XCircle, Loader2, User, Bot, Brain, Play, Square,
  FilePlus, FilePen, FileX, FileSymlink, RefreshCw, FolderGit2, Gauge,
} from "lucide-react";
import Markdown from "react-markdown";
import rehypeHighlight from "rehype-highlight";
import { DiffViewer } from "../components/DiffViewer";
import { ipc, type GitStatus, type GitFileStatus } from "../lib/ipc";

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
    reasoningLevels,
    lastUsage,
    modelContextLengths,
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
    setReasoning,
    loadModelContext,
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
  const reasoning = active ? reasoningLevels[active.id] ?? "auto" : "auto";
  const usage = active ? lastUsage[active.id] : undefined;
  const usedTokens = usage ? usage.tokensIn + usage.tokensOut : 0;
  const ctxLen = selectedModel ? modelContextLengths[selectedModel] ?? null : null;
  const ctxPct =
    ctxLen && ctxLen > 0 ? Math.min(100, Math.round((usedTokens / ctxLen) * 100)) : null;

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

  // Charge la taille de contexte du modèle sélectionné (pour l'indicateur).
  useEffect(() => {
    if (selectedModel) void loadModelContext(selectedModel);
  }, [selectedModel, loadModelContext]);

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
    <div className="flex flex-1 overflow-hidden">
      {/* Bloc 1 — conversation (50%) */}
      <div className="flex w-1/2 flex-col overflow-hidden">
      <header className="flex items-center gap-2 border-b border-border px-4 py-2 text-xs text-muted">
        <span>ollama</span>
        <span>•</span>
        <span className="truncate font-mono" title={active.workspace}>
          {active.workspace}
        </span>
      </header>

      <div className="flex-1 overflow-y-auto px-4 py-4">
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
          return (
            <div key={i} className="mb-4 text-sm">
              <div className="mb-1 flex items-center gap-1.5 text-xs font-semibold text-muted">
                {meta.icon}
                {meta.label}
              </div>
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
        <div className="mb-2 flex flex-wrap items-center gap-2 text-xs">
          {hasModels ? (
            <select
              value={selectedModel}
              onChange={(e) => setSelectedModel(active.id, e.target.value)}
              disabled={isStreaming}
              title="Modèle"
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
              Aucun modèle installé — voir l'onglet Ollama
            </span>
          )}

          <label className="flex items-center gap-1 text-muted" title="Intensité de raisonnement (modèles « thinking »)">
            <Brain size={12} className="text-purple-400" />
            <select
              value={reasoning}
              onChange={(e) => setReasoning(active.id, e.target.value)}
              disabled={isStreaming}
              className="rounded border border-border bg-surface px-2 py-1 text-xs text-fg focus:border-accent focus:outline-none disabled:opacity-50"
            >
              <option value="auto">Raisonnement : Auto</option>
              <option value="off">Désactivé</option>
              <option value="low">Faible</option>
              <option value="medium">Moyen</option>
              <option value="high">Élevé</option>
            </select>
          </label>

          <div
            className="ml-auto flex items-center gap-1.5 text-muted"
            title={
              ctxLen
                ? `Contexte utilisé au dernier tour : ${usedTokens} / ${ctxLen} tokens`
                : "Usage de contexte (après le premier échange)"
            }
          >
            <Gauge size={12} />
            {ctxLen ? (
              <>
                <span className="tabular-nums">
                  {usedTokens.toLocaleString()} / {ctxLen.toLocaleString()}
                </span>
                <div className="h-1.5 w-16 overflow-hidden rounded bg-border/50">
                  <div
                    className={`h-full ${(ctxPct ?? 0) >= 90 ? "bg-red-500" : "bg-accent"}`}
                    style={{ width: `${ctxPct ?? 0}%` }}
                  />
                </div>
                <span className="w-8 text-right tabular-nums">{ctxPct}%</span>
              </>
            ) : (
              <span className="tabular-nums">
                {usedTokens > 0 ? `${usedTokens.toLocaleString()} tokens` : "contexte —"}
              </span>
            )}
          </div>
        </div>

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
              title="Arrêter la génération"
              aria-label="Arrêter la génération"
              className="flex items-center justify-center rounded border border-red-500/40 p-2.5 text-red-300 hover:bg-red-500/10"
            >
              <Square size={16} className="fill-current" />
            </button>
          ) : (
            <button
              onClick={submit}
              disabled={!input.trim() || !selectedModel}
              title={!selectedModel ? "Sélectionnez un modèle" : "Envoyer"}
              aria-label="Envoyer"
              className="flex items-center justify-center rounded bg-accent p-2.5 text-white hover:bg-accent/80 disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:bg-accent"
            >
              <Play size={16} className="fill-current" />
            </button>
          )}
        </div>
      </div>
      </div>

      {/* Bloc 2 — raisonnement (25%) */}
      <ThinkingPanel thinking={activeThinking} isStreaming={isStreaming} />

      {/* Bloc 3 — fichiers du workspace (25%) */}
      <FileChangesPanel workspace={active.workspace} isStreaming={isStreaming} />
    </div>
  );
}

/// Panneau latéral affichant le raisonnement (thinking) du modèle en temps réel.
function ThinkingPanel({
  thinking,
  isStreaming,
}: {
  thinking: string;
  isStreaming: boolean;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);

  // Auto-scroll pendant le streaming.
  useEffect(() => {
    if (isStreaming && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [thinking, isStreaming]);

  return (
    <aside className="flex w-1/4 shrink-0 flex-col overflow-hidden border-l border-border bg-bg">
      <div className="flex items-center gap-2 border-b border-border px-3 py-2 text-xs">
        <Brain size={14} className="text-purple-400" />
        <span className="font-semibold text-fg">Raisonnement</span>
        {isStreaming && thinking && (
          <Loader2 size={12} className="ml-auto animate-spin text-purple-400" />
        )}
      </div>

      <div ref={scrollRef} className="flex-1 overflow-y-auto px-3 py-2 text-xs">
        {!thinking && !isStreaming && (
          <p className="px-1 py-2 text-muted">
            Le raisonnement du modèle apparaîtra ici pendant la génération.
          </p>
        )}
        {!thinking && isStreaming && (
          <div className="flex items-center gap-2 px-1 py-2 text-muted">
            <Brain size={12} className="animate-pulse text-purple-400" />
            <span>En train de réfléchir…</span>
          </div>
        )}
        {thinking && (
          <div className="whitespace-pre-wrap text-fg/80">
            {thinking}
          </div>
        )}
      </div>
    </aside>
  );
}

/// Métadonnées d'affichage par statut de fichier git.
const GIT_STATUS_META: Record<
  GitFileStatus,
  { label: string; icon: React.ReactNode; color: string }
> = {
  added: { label: "Ajouté", icon: <FilePlus size={13} />, color: "text-green-400" },
  untracked: { label: "Nouveau", icon: <FilePlus size={13} />, color: "text-green-400" },
  modified: { label: "Modifié", icon: <FilePen size={13} />, color: "text-yellow-400" },
  deleted: { label: "Supprimé", icon: <FileX size={13} />, color: "text-red-400" },
  renamed: { label: "Renommé", icon: <FileSymlink size={13} />, color: "text-blue-400" },
};

/// Panneau latéral listant les fichiers modifiés du workspace via git.
function FileChangesPanel({
  workspace,
  isStreaming,
}: {
  workspace: string;
  isStreaming: boolean;
}) {
  const [status, setStatus] = useState<GitStatus | null>(null);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(() => {
    setLoading(true);
    ipc
      .workspaceGitStatus({ workspace })
      .then(setStatus)
      .catch(() => setStatus(null))
      .finally(() => setLoading(false));
  }, [workspace]);

  // Rafraîchit au changement de workspace et quand le streaming bascule.
  // Pendant une génération, on interroge git périodiquement pour un suivi live.
  useEffect(() => {
    refresh();
    if (!isStreaming) return;
    const id = setInterval(refresh, 2500);
    return () => clearInterval(id);
  }, [workspace, isStreaming, refresh]);

  const changes = status?.changes ?? [];

  return (
    <aside className="flex w-1/4 shrink-0 flex-col overflow-hidden border-l border-border bg-bg">
      <div className="flex items-center gap-2 border-b border-border px-3 py-2 text-xs">
        <FolderGit2 size={14} className="text-accent" />
        <span className="font-semibold text-fg">Modifications</span>
        {changes.length > 0 && (
          <span className="rounded bg-border/60 px-1.5 py-0.5 text-[10px] text-muted">
            {changes.length}
          </span>
        )}
        <button
          onClick={refresh}
          title="Rafraîchir"
          aria-label="Rafraîchir"
          className="ml-auto rounded p-1 text-muted hover:bg-border/40 hover:text-fg"
        >
          <RefreshCw size={13} className={loading ? "animate-spin" : ""} />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto px-2 py-2 text-xs">
        {status === null ? (
          <p className="px-1 py-2 text-muted">git indisponible sur ce répertoire.</p>
        ) : !status.isRepo ? (
          <p className="px-1 py-2 text-muted">
            Répertoire non versionné (pas un dépôt git).
          </p>
        ) : changes.length === 0 ? (
          <p className="px-1 py-2 text-muted">Aucune modification.</p>
        ) : (
          <ul className="space-y-0.5">
            {changes.map((c) => {
              const meta = GIT_STATUS_META[c.status];
              const name = c.path.split("/").pop() ?? c.path;
              const dir = c.path.slice(0, c.path.length - name.length);
              return (
                <li
                  key={c.path}
                  className="flex items-center gap-1.5 rounded px-1.5 py-1 hover:bg-border/30"
                  title={`${meta.label} — ${c.path}`}
                >
                  <span className={meta.color}>{meta.icon}</span>
                  <span className="truncate text-fg">{name}</span>
                  {dir && (
                    <span className="truncate text-[10px] text-muted">{dir}</span>
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </aside>
  );
}
