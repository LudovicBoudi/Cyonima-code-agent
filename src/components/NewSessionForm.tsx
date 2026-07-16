import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type ProviderId = "llama_cpp" | "ollama" | "openai" | "anthropic" | "gemini" | "openai_compat";

export function NewSessionForm({
  onCreate,
  onCancel,
}: {
  onCreate: (p: { workspace: string; modelId: string; providerId: ProviderId }) => void;
  onCancel: () => void;
}) {
  const [workspace, setWorkspace] = useState(".");
  const [modelId, setModelId] = useState("llama3.2");
  const [providerId, setProviderId] = useState<ProviderId>("ollama");
  const [hwInfo, setHwInfo] = useState<{ total_ram_gb: number; cpu_cores: number; os: string } | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    invoke<{ total_ram_gb: number; cpu_cores: number; os: string }>("hardware_get")
      .then(setHwInfo)
      .catch(() => null);
  }, []);

  const submit = () => {
    if (!modelId.trim()) return;
    setBusy(true);
    onCreate({ workspace: workspace.trim() || ".", modelId: modelId.trim(), providerId });
  };

  return (
    <div className="flex h-full items-center justify-center p-6">
      <div className="w-full max-w-lg rounded border border-border bg-bg p-6">
        <h2 className="mb-4 text-base font-semibold">Nouvelle session</h2>

        {hwInfo && (
          <p className="mb-4 text-xs text-muted">
            Hôte : {hwInfo.os}, {hwInfo.cpu_cores} cœurs, {hwInfo.total_ram_gb} Go RAM
          </p>
        )}

        <label className="mb-1 block text-xs text-muted">Provider</label>
        <select
          value={providerId}
          onChange={(e) => setProviderId(e.target.value as ProviderId)}
          className="mb-3 w-full rounded border border-border bg-bg px-2 py-1 text-sm"
        >
          <option value="ollama">Ollama (local, recommandé pour J1)</option>
          <option value="llama_cpp">llama_cpp (built-in — J1.5)</option>
          <option value="openai">OpenAI (J6)</option>
          <option value="anthropic">Anthropic (J6)</option>
          <option value="gemini">Gemini (J6)</option>
          <option value="openai_compat">OpenAI-compat (J6)</option>
        </select>

        <label className="mb-1 block text-xs text-muted">
          Modèle {providerId === "ollama" ? "(nom Ollama, ex: llama3.2)" : "(identifiant)"}
        </label>
        <input
          value={modelId}
          onChange={(e) => setModelId(e.target.value)}
          className="mb-3 w-full rounded border border-border bg-bg px-2 py-1 text-sm"
          placeholder="llama3.2"
        />

        <label className="mb-1 block text-xs text-muted">Workspace (chemin)</label>
        <input
          value={workspace}
          onChange={(e) => setWorkspace(e.target.value)}
          className="mb-4 w-full rounded border border-border bg-bg px-2 py-1 text-sm font-mono"
          placeholder="."
        />

        <div className="flex gap-2">
          <button
            onClick={submit}
            disabled={busy}
            className="rounded bg-accent px-3 py-1.5 text-xs text-white disabled:opacity-50"
          >
            {busy ? "Création…" : "Lancer la session"}
          </button>
          <button
            onClick={onCancel}
            className="rounded border border-border px-3 py-1.5 text-xs text-muted hover:bg-border/40"
          >
            Annuler
          </button>
        </div>

        <p className="mt-4 text-xs text-muted">
          Pour tester immédiatement : installez Ollama (ollama.com), lancez <code>ollama serve</code> dans un
          terminal et <code>ollama pull llama3.2</code> dans un autre, puis créez la session ci-dessus.
        </p>
      </div>
    </div>
  );
}