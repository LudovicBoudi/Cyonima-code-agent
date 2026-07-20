import { useEffect, useState } from "react";
import { ipc } from "../lib/ipc";
import type { GlobalConfig } from "../lib/ipc";
import { Settings, CheckCircle, AlertCircle } from "lucide-react";

const TOOLS = ["read_file", "write_file", "edit_file", "glob", "grep", "bash"];
const POLICIES = ["auto", "ask", "deny"];

export function ConfigView() {
  const [config, setConfig] = useState<GlobalConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [feedback, setFeedback] = useState<{ ok: boolean; msg: string } | null>(null);

  const loadConfig = async () => {
    setLoading(true);
    try {
      const cfg = await ipc.configGet();
      setConfig(cfg);
    } catch (e) {
      setFeedback({ ok: false, msg: String(e) });
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void loadConfig();
  }, []);

  const showFeedback = (ok: boolean, msg: string) => {
    setFeedback({ ok, msg });
    setTimeout(() => setFeedback(null), 3000);
  };

  const handleProviderChange = async (value: string) => {
    try {
      await ipc.configSetDefaultProvider({ provider: value || null });
      showFeedback(true, "Provider par défaut enregistré.");
      await loadConfig();
    } catch (e) {
      showFeedback(false, String(e));
    }
  };

  const handleModelChange = async (value: string) => {
    try {
      await ipc.configSetDefaultModel({ model: value || null });
      showFeedback(true, "Modèle par défaut enregistré.");
      await loadConfig();
    } catch (e) {
      showFeedback(false, String(e));
    }
  };

  const handleEndpointChange = async (value: string) => {
    try {
      await ipc.configSetOllamaEndpoint({ endpoint: value || null });
      showFeedback(true, "Endpoint Ollama enregistré.");
      await loadConfig();
    } catch (e) {
      showFeedback(false, String(e));
    }
  };

  const handlePermissionChange = async (tool: string, policy: string) => {
    try {
      await ipc.configSetPermission({ tool, policy });
      showFeedback(true, `Permission "${tool}" → ${policy}.`);
      await loadConfig();
    } catch (e) {
      showFeedback(false, String(e));
    }
  };

  const handlePermissionRemove = async (tool: string) => {
    try {
      await ipc.configRemovePermission({ tool });
      showFeedback(true, `Override "${tool}" supprimé.`);
      await loadConfig();
    } catch (e) {
      showFeedback(false, String(e));
    }
  };

  if (loading || !config) {
    return (
      <div className="flex flex-1 items-center justify-center p-8 text-sm text-muted">
        Chargement…
      </div>
    );
  }

  return (
    <div className="flex flex-1 flex-col overflow-y-auto p-8">
      <div className="mx-auto w-full max-w-lg space-y-8">
        <div className="flex items-center gap-3">
          <Settings size={24} className="text-accent" />
          <h2 className="text-lg font-semibold text-fg">Configuration</h2>
        </div>

        <p className="text-sm text-muted">
          Config globale <code>~/.cyonima/config.toml</code>. Les overrides par projet
          (<code>&lt;workspace&gt;/.cyonima/config.toml</code>) surchargent ces valeurs.
        </p>

        {feedback && (
          <div
            className={`flex items-center gap-1.5 rounded border p-2 text-sm ${
              feedback.ok
                ? "border-green-600/40 bg-green-900/20 text-green-300"
                : "border-red-600/40 bg-red-900/20 text-red-300"
            }`}
          >
            {feedback.ok ? <CheckCircle size={14} /> : <AlertCircle size={14} />}
            {feedback.msg}
          </div>
        )}

        {/* Provider & model defaults */}
        <section className="space-y-4">
          <h3 className="text-sm font-medium text-fg">Provider & modèle par défaut</h3>

          <div className="space-y-2">
            <label className="text-xs text-muted">Provider par défaut</label>
            <select
              value={config.provider.defaultProvider ?? ""}
              onChange={(e) => void handleProviderChange(e.target.value)}
              className="w-full rounded border border-border bg-bg px-3 py-2 text-sm text-fg focus:border-accent focus:outline-none"
            >
              <option value="">Aucun (choix manuel)</option>
              <option value="ollama">Ollama</option>
              <option value="openai">OpenAI</option>
              <option value="anthropic">Anthropic</option>
              <option value="gemini">Gemini</option>
              <option value="openai_compat">OpenAI-compat</option>
              <option value="llama_cpp">llama.cpp</option>
            </select>
          </div>

          <div className="space-y-2">
            <label className="text-xs text-muted">Modèle par défaut</label>
            <input
              type="text"
              placeholder="ex: gemma4:12b, gpt-4o, claude-sonnet-4-20250514"
              value={config.provider.defaultModel ?? ""}
              onChange={(e) => void handleModelChange(e.target.value)}
              className="w-full rounded border border-border bg-bg px-3 py-2 text-sm text-fg placeholder:text-muted/60 focus:border-accent focus:outline-none"
            />
          </div>

          <div className="space-y-2">
            <label className="text-xs text-muted">Endpoint Ollama</label>
            <input
              type="text"
              placeholder="http://localhost:11434"
              value={config.provider.ollamaEndpoint ?? ""}
              onChange={(e) => void handleEndpointChange(e.target.value)}
              className="w-full rounded border border-border bg-bg px-3 py-2 text-sm text-fg placeholder:text-muted/60 focus:border-accent focus:outline-none"
            />
          </div>
        </section>

        {/* Permissions overrides */}
        <section className="space-y-4">
          <h3 className="text-sm font-medium text-fg">Permissions des outils</h3>
          <p className="text-xs text-muted">
            Par défaut : read/glob/grep = <strong>Auto</strong>, write/edit/bash = <strong>Ask</strong>.
            Les overrides ci-dessous changent le comportement pour ce workspace.
          </p>

          <div className="space-y-2">
            {TOOLS.map((tool) => {
              const current = config.permissions.overrides[tool];
              return (
                <div key={tool} className="flex items-center gap-3">
                  <span className="w-28 text-sm text-fg font-mono">{tool}</span>
                  <select
                    value={current ?? ""}
                    onChange={(e) =>
                      e.target.value
                        ? void handlePermissionChange(tool, e.target.value)
                        : void handlePermissionRemove(tool)
                    }
                    className="flex-1 rounded border border-border bg-bg px-2 py-1.5 text-xs text-fg focus:border-accent focus:outline-none"
                  >
                    <option value="">Défaut</option>
                    {POLICIES.map((p) => (
                      <option key={p} value={p}>
                        {p}
                      </option>
                    ))}
                  </select>
                  {current && (
                    <button
                      onClick={() => void handlePermissionRemove(tool)}
                      className="text-xs text-muted hover:text-red-400"
                    >
                      reset
                    </button>
                  )}
                </div>
              );
            })}
          </div>
        </section>
      </div>
    </div>
  );
}
