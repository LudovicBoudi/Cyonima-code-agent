import { useSessionsStore } from "../store/sessions";
import { Plus, Terminal } from "lucide-react";

const PROVIDER_LABEL: Record<string, string> = {
  llama_cpp: "llama.cpp",
  ollama: "Ollama",
  openai: "OpenAI",
  anthropic: "Anthropic",
  gemini: "Gemini",
  openai_compat: "OpenAI-compat",
};

export function Sidebar() {
  const { sessions, activeSessionId, setActive, startCreating } = useSessionsStore();

  return (
    <aside className="flex w-60 shrink-0 flex-col border-r border-border bg-bg">
      <div className="flex items-center gap-2 px-4 py-3 text-sm font-semibold">
        <Terminal size={16} className="text-accent" />
        Cyonima
      </div>
      <nav className="flex-1 overflow-y-auto px-2 py-2">
        {sessions.length === 0 && (
          <p className="px-2 py-4 text-xs text-muted">Aucune session. Cliquez + pour en lancer une.</p>
        )}
        {sessions.map((s) => (
          <button
            key={s.id}
            onClick={() => setActive(s.id)}
            className={`mb-1 w-full rounded px-2 py-2 text-left text-xs ${
              s.id === activeSessionId ? "bg-accent/20 text-fg" : "text-muted hover:bg-border/40"
            }`}
          >
            <div className="truncate font-medium">{s.modelId}</div>
            <div className="truncate text-muted">{PROVIDER_LABEL[s.providerId] ?? s.providerId}</div>
          </button>
        ))}
      </nav>
      <button
        onClick={startCreating}
        className="m-2 flex items-center gap-2 rounded border border-border px-2 py-2 text-xs text-muted hover:bg-border/40"
        title="Nouvelle session"
      >
        <Plus size={14} /> Nouvelle session
      </button>
    </aside>
  );
}