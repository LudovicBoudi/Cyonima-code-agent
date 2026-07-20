import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { ipc } from "../lib/ipc";
import { FolderOpen } from "lucide-react";

export function NewSessionForm({
  onCreate,
  onCancel,
}: {
  onCreate: (p: { workspace: string }) => void;
  onCancel: () => void;
}) {
  const [workspace, setWorkspace] = useState(".");
  const [hwInfo, setHwInfo] = useState<{
    totalRamGb: number;
    cpuCores: number;
    os: string;
    vramGb: number;
  } | null>(null);

  useEffect(() => {
    ipc.hardwareGet().then(setHwInfo).catch(() => null);
  }, []);

  const pickFolder = async () => {
    const selected = await open({ directory: true });
    if (typeof selected === "string") {
      setWorkspace(selected);
    }
  };

  const submit = () => {
    onCreate({ workspace: workspace.trim() || "." });
  };

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
        <div className="mb-2 flex gap-2">
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

        <p className="mb-4 text-xs text-muted">
          Le modèle se choisit dans le menu déroulant du chat, parmi les modèles
          installés dans Ollama.
        </p>

        <div className="flex gap-2">
          <button
            onClick={submit}
            className="rounded bg-accent px-3 py-1.5 text-xs text-white"
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
