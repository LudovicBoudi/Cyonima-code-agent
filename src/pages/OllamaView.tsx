import { useEffect, useState } from "react";
import { ipc, onOllamaPullProgress, onOllamaPullDone, onOllamaPullError } from "../lib/ipc";
import type { OllamaModelInfo } from "../lib/ipc";
import { Download, RefreshCw, CheckCircle, AlertCircle, Server } from "lucide-react";

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 o";
  const k = 1024;
  const units = ["o", "Ko", "Mo", "Go", "To"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${units[i]}`;
}

const POPULAR_MODELS = [
  { name: "gemma4:12b", label: "Gemma 4 12B", desc: "Coding agent par défaut" },
  { name: "gemma4:e2b", label: "Gemma 4 E2B", desc: "Edge, 3.4 Go" },
  { name: "gemma4:e4b", label: "Gemma 4 E4B", desc: "Laptop standard" },
  { name: "gemma4:26b", label: "Gemma 4 26B", desc: "MoE, long-context" },
  { name: "qwen2.5-coder:7b", label: "Qwen Coder 7B", desc: "Coding budget" },
  { name: "llama3.1:8b", label: "Llama 3.1 8B", desc: "Généraliste" },
  { name: "mistral:7b", label: "Mistral 7B", desc: "Densité efficace" },
  { name: "deepseek-r1:8b", label: "DeepSeek R1 8B", desc: "Reasoning" },
];

export function OllamaView() {
  const [models, setModels] = useState<OllamaModelInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [pulling, setPulling] = useState<string | null>(null);
  const [pullStatus, setPullStatus] = useState<string>("");
  const [pullDone, setPullDone] = useState<string | null>(null);
  const [pullError, setPullError] = useState<string | null>(null);
  const [customModel, setCustomModel] = useState("");

  const loadModels = async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await ipc.ollamaListModels();
      setModels(list);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void loadModels();
  }, []);

  // Event listeners for pull progress
  useEffect(() => {
    const unlistens: Array<() => void> = [];
    (async () => {
      unlistens.push(
        await onOllamaPullProgress((e) => {
          setPullStatus(e.status || "");
        }),
      );
      unlistens.push(
        await onOllamaPullDone((e) => {
          setPullDone(e.model);
          setPulling(null);
          setPullStatus("");
          void loadModels();
        }),
      );
      unlistens.push(
        await onOllamaPullError((e) => {
          setPullError(e.error);
          setPulling(null);
          setPullStatus("");
        }),
      );
    })();
    return () => unlistens.forEach((u) => u());
  }, []);

  const handlePull = async (model: string) => {
    setPulling(model);
    setPullStatus("");
    setPullDone(null);
    setPullError(null);
    try {
      await ipc.ollamaPullModel({ model });
    } catch (e) {
      setPullError(String(e));
      setPulling(null);
    }
  };

  const handlePullCustom = async () => {
    const m = customModel.trim();
    if (!m) return;
    await handlePull(m);
    setCustomModel("");
  };

  return (
    <div className="flex flex-1 flex-col overflow-y-auto p-8">
      <div className="mx-auto w-full max-w-2xl space-y-8">
        <div className="flex items-center gap-3">
          <Server size={24} className="text-accent" />
          <h2 className="text-lg font-semibold text-fg">Ollama — modèles locaux</h2>
        </div>

        <p className="text-sm text-muted">
          Ollama doit être lancé (<code>ollama serve</code>) pour que cette page fonctionne.
          Les modèles listés ici sont déjà téléchargés côté Ollama et utilisables directement
          en créant une session avec le provider <strong>Ollama</strong>.
        </p>

        {/* Installed models */}
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-medium text-fg">Installés ({models.length})</h3>
            <button
              onClick={() => void loadModels()}
              disabled={loading}
              className="flex items-center gap-1.5 rounded border border-border px-2 py-1 text-xs text-muted hover:bg-border/40 disabled:opacity-50"
            >
              <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
              Rafraîchir
            </button>
          </div>

          {error && (
            <div className="flex items-start gap-2 rounded border border-red-600/40 bg-red-900/20 p-3 text-sm text-red-300">
              <AlertCircle size={16} className="mt-0.5 shrink-0" />
              <p>{error}</p>
            </div>
          )}

          {!loading && models.length === 0 && !error && (
            <p className="rounded border border-border bg-bg p-4 text-center text-sm text-muted">
              Aucun modèle installé. Utilisez la section ci-dessous pour en télécharger.
            </p>
          )}

          {models.map((m) => (
            <div
              key={m.name}
              className="flex items-center justify-between rounded border border-border bg-surface px-4 py-3"
            >
              <div>
                <div className="text-sm font-medium text-fg">{m.name}</div>
                <div className="text-xs text-muted">
                  {formatBytes(m.size)} · {m.digest.slice(0, 12)}…
                </div>
              </div>
              <span className="rounded bg-green-900/30 px-2 py-0.5 text-[10px] text-green-400">
                prêt
              </span>
            </div>
          ))}
        </div>

        {/* Pull new model */}
        <div className="space-y-3">
          <h3 className="text-sm font-medium text-fg">Télécharger un modèle</h3>

          {/* Popular models grid */}
          <div className="grid grid-cols-2 gap-2">
            {POPULAR_MODELS.map((pm) => {
              const isInstalled = models.some((m) => m.name === pm.name || m.name.startsWith(pm.name + ":"));
              const isPulling = pulling === pm.name;
              return (
                <button
                  key={pm.name}
                  onClick={() => void handlePull(pm.name)}
                  disabled={isInstalled || isPulling}
                  className={`flex flex-col items-start rounded border px-3 py-2 text-left text-xs transition ${
                    isInstalled
                      ? "border-green-600/30 bg-green-900/10 opacity-60"
                      : isPulling
                        ? "border-accent bg-accent/10"
                        : "border-border hover:border-accent/50 hover:bg-border/20"
                  }`}
                >
                  <div className="flex items-center gap-1.5">
                    {isInstalled ? (
                      <CheckCircle size={12} className="text-green-400" />
                    ) : isPulling ? (
                      <RefreshCw size={12} className="animate-spin text-accent" />
                    ) : (
                      <Download size={12} className="text-muted" />
                    )}
                    <span className="font-medium text-fg">{pm.label}</span>
                  </div>
                  <span className="mt-0.5 text-muted">{pm.desc}</span>
                  <span className="mt-0.5 text-muted/60">{pm.name}</span>
                </button>
              );
            })}
          </div>

          {/* Custom model input */}
          <div className="flex gap-2">
            <input
              type="text"
              placeholder="nom-du-modèle:tag (ex: gemma4:12b)"
              value={customModel}
              onChange={(e) => setCustomModel(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && void handlePullCustom()}
              className="flex-1 rounded border border-border bg-bg px-3 py-2 text-sm text-fg placeholder:text-muted/60 focus:border-accent focus:outline-none"
            />
            <button
              onClick={() => void handlePullCustom()}
              disabled={!customModel.trim() || pulling !== null}
              className="flex items-center gap-1.5 rounded bg-accent px-3 py-2 text-sm font-medium text-bg disabled:opacity-50 hover:opacity-90"
            >
              <Download size={14} />
              Pull
            </button>
          </div>

          {/* Pull progress */}
          {pulling && (
            <div className="rounded border border-accent/40 bg-accent/5 p-3 text-sm text-fg">
              <div className="flex items-center gap-2">
                <RefreshCw size={14} className="animate-spin text-accent" />
                <span className="font-medium">{pulling}</span>
              </div>
              {pullStatus && (
                <p className="mt-1 text-xs text-muted">{pullStatus}</p>
              )}
            </div>
          )}

          {pullDone && (
            <div className="flex items-center gap-2 rounded border border-green-600/40 bg-green-900/20 p-3 text-sm text-green-300">
              <CheckCircle size={14} />
              <span><strong>{pullDone}</strong> prêt à l'usage.</span>
            </div>
          )}

          {pullError && (
            <div className="flex items-start gap-2 rounded border border-red-600/40 bg-red-900/20 p-3 text-sm text-red-300">
              <AlertCircle size={16} className="mt-0.5 shrink-0" />
              <p>{pullError}</p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
