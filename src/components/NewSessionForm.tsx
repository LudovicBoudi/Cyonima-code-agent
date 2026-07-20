import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { ipc, type ModelInfo } from "../lib/ipc";
import { FolderOpen, Download, HardDrive, Globe, Check } from "lucide-react";

type ProviderId = "llama_cpp" | "ollama" | "openai" | "anthropic" | "gemini" | "openai_compat";

const PROVIDER_LABEL: Record<ProviderId, string> = {
  llama_cpp: "Local (llama.cpp)",
  ollama: "Ollama",
  openai: "OpenAI",
  anthropic: "Anthropic",
  gemini: "Gemini",
  openai_compat: "OpenAI-compat",
};

function inferProvider(model: ModelInfo): ProviderId {
  if (model.ollamaTag) return "ollama";
  if (model.installed && model.installedPath) return "llama_cpp";
  return "ollama";
}

function formatSize(bytes: number): string {
  if (bytes === 0) return "";
  const gb = bytes / (1024 ** 3);
  return gb >= 1 ? `${gb.toFixed(1)} Go` : `${(bytes / (1024 ** 2)).toFixed(0)} Mo`;
}

export function NewSessionForm({
  onCreate,
  onCancel,
}: {
  onCreate: (p: { workspace: string; modelId: string; providerId: ProviderId }) => void;
  onCancel: () => void;
}) {
  const [workspace, setWorkspace] = useState(".");
  const [selectedModel, setSelectedModel] = useState<ModelInfo | null>(null);
  const [modelIdInput, setModelIdInput] = useState("");
  const [providerId, setProviderId] = useState<ProviderId>("ollama");
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [hwInfo, setHwInfo] = useState<{ totalRamGb: number; cpuCores: number; os: string; vramGb: number } | null>(null);

  useEffect(() => {
    Promise.all([
      ipc.modelListInstalled().catch(() => [] as ModelInfo[]),
      ipc.modelCatalogList().catch(() => [] as ModelInfo[]),
      ipc.hardwareGet().catch(() => null),
    ]).then(([installed, catalog, hw]) => {
      // Fusionner : installés en premier, puis le reste du catalogue
      const installedIds = new Set(installed.map((m) => m.id));
      const catalogOnly = catalog.filter((m) => !installedIds.has(m.id));
      // Trier les installés par taille décroissante
      const sortedInstalled = installed.sort((a, b) => b.sizeBytes - a.sizeBytes);
      setModels([...sortedInstalled, ...catalogOnly]);
      setHwInfo(hw);
      setLoading(false);
    });
  }, []);

  const handleSelectModel = (model: ModelInfo) => {
    setSelectedModel(model);
    setModelIdInput(model.ollamaTag ?? model.id);
    setProviderId(inferProvider(model));
  };

  const pickFolder = async () => {
    const selected = await open({ directory: true });
    if (typeof selected === "string") {
      setWorkspace(selected);
    }
  };

  const submit = () => {
    const id = selectedModel
      ? (selectedModel.ollamaTag ?? selectedModel.id)
      : modelIdInput.trim();
    if (!id) return;
    onCreate({ workspace: workspace.trim() || ".", modelId: id, providerId });
  };

  const installed = models.filter((m) => m.installed);
  const available = models.filter((m) => !m.installed);

  return (
    <div className="flex h-full items-center justify-center p-6">
      <div className="w-full max-w-lg rounded border border-border bg-bg p-6">
        <h2 className="mb-4 text-base font-semibold">Nouvelle session</h2>

        {hwInfo && (
          <p className="mb-4 text-xs text-muted">
            {hwInfo.os}, {hwInfo.cpuCores} cœurs, {hwInfo.totalRamGb} Go RAM
            {hwInfo.vramGb > 0 && ` + ${hwInfo.vramGb} Go VRAM`}
          </p>
        )}

        {/* Workspace */}
        <label className="mb-1 block text-xs text-muted">Répertoire de travail</label>
        <div className="flex gap-2 mb-4">
          <input
            value={workspace}
            onChange={(e) => setWorkspace(e.target.value)}
            className="flex-1 rounded border border-border bg-surface px-2 py-1.5 text-sm font-mono"
            placeholder="."
          />
          <button
            onClick={() => void pickFolder()}
            className="flex items-center gap-1.5 rounded border border-border bg-surface px-3 py-1.5 text-xs text-muted hover:bg-border/40"
          >
            <FolderOpen size={14} />
            Parcourir
          </button>
        </div>

        {/* Modèles installés */}
        {installed.length > 0 && (
          <>
            <label className="mb-1 flex items-center gap-1.5 text-xs text-muted">
              <HardDrive size={12} />
              Modèles installés ({installed.length})
            </label>
            <div className="mb-3 max-h-48 overflow-y-auto rounded border border-border">
              {installed.map((m) => (
                <button
                  key={m.id}
                  onClick={() => handleSelectModel(m)}
                  className={`flex w-full items-center gap-2 border-b border-border/40 px-3 py-2 text-left text-xs last:border-b-0 ${
                    selectedModel?.id === m.id
                      ? "bg-accent/20 text-fg"
                      : "hover:bg-border/40 text-fg"
                  }`}
                >
                  <Check size={12} className="shrink-0 text-green-400" />
                  <div className="flex-1 truncate">
                    <span className="font-medium">{m.name}</span>
                    <span className="ml-2 text-muted">{m.quantization}</span>
                  </div>
                  <span className="shrink-0 rounded bg-green-500/20 px-1.5 py-0.5 text-[10px] font-medium text-green-400">
                    Local
                  </span>
                  {m.sizeBytes > 0 && (
                    <span className="shrink-0 text-muted">{formatSize(m.sizeBytes)}</span>
                  )}
                </button>
              ))}
            </div>
          </>
        )}

        {/* Modèles du catalogue (non installés) */}
        {available.length > 0 && (
          <>
            <label className="mb-1 flex items-center gap-1.5 text-xs text-muted">
              <Globe size={12} />
              Catalogue ({available.length})
            </label>
            <div className="mb-3 max-h-48 overflow-y-auto rounded border border-border">
              {available.map((m) => (
                <button
                  key={m.id}
                  onClick={() => handleSelectModel(m)}
                  className={`flex w-full items-center gap-2 border-b border-border/40 px-3 py-2 text-left text-xs last:border-b-0 ${
                    selectedModel?.id === m.id
                      ? "bg-accent/20 text-fg"
                      : "hover:bg-border/40 text-fg"
                  }`}
                >
                  <Download size={12} className="shrink-0 text-muted" />
                  <div className="flex-1 truncate">
                    <span className="font-medium">{m.name}</span>
                    <span className="ml-2 text-muted">{m.quantization}</span>
                  </div>
                  {m.ramMinGb > 0 && (
                    <span className="shrink-0 text-muted">{m.ramMinGb} Go</span>
                  )}
                </button>
              ))}
            </div>
          </>
        )}

        {loading && (
          <p className="mb-3 text-xs text-muted">Chargement des modèles…</p>
        )}

        {/* Provider */}
        <label className="mb-1 block text-xs text-muted">Provider</label>
        <select
          value={providerId}
          onChange={(e) => setProviderId(e.target.value as ProviderId)}
          className="mb-3 w-full rounded border border-border bg-surface px-2 py-1.5 text-sm"
        >
          {(Object.entries(PROVIDER_LABEL) as [ProviderId, string][]).map(([id, label]) => (
            <option key={id} value={id}>{label}</option>
          ))}
        </select>

        {/* Modèle manuel */}
        <label className="mb-1 block text-xs text-muted">
          {selectedModel ? "Modèle sélectionné" : "Modèle (identifiant ou tag Ollama)"}
        </label>
        <input
          value={selectedModel ? (selectedModel.ollamaTag ?? selectedModel.id) : modelIdInput}
          onChange={(e) => {
            setModelIdInput(e.target.value);
            setSelectedModel(null);
          }}
          className="mb-4 w-full rounded border border-border bg-surface px-2 py-1.5 text-sm"
          placeholder="ex: qwen2.5:7b"
        />

        <div className="flex gap-2">
          <button
            onClick={submit}
            disabled={(!selectedModel && !modelIdInput.trim())}
            className="rounded bg-accent px-3 py-1.5 text-xs text-white disabled:opacity-50"
          >
            Lancer la session
          </button>
          <button
            onClick={onCancel}
            className="rounded border border-border px-3 py-1.5 text-xs text-muted hover:bg-border/40"
          >
            Annuler
          </button>
        </div>
      </div>
    </div>
  );
}
