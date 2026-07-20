import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { ipc } from "../lib/ipc";
import { Upload, CheckCircle, AlertCircle, FolderOpen } from "lucide-react";

export function ImportModelView() {
  const [path, setPath] = useState("");
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<{ ok: boolean; message: string; modelId?: string } | null>(null);

  const pickFile = async () => {
    const selected = await open({
      multiple: false,
      filters: [
        { name: "GGUF", extensions: ["gguf"] },
      ],
    });
    if (typeof selected === "string") {
      setPath(selected);
      setResult(null);
    }
  };

  const handleImport = async () => {
    if (!path.trim()) return;
    setLoading(true);
    setResult(null);
    try {
      const info = await ipc.modelImportCustom({ path: path.trim() });
      setResult({ ok: true, message: `Modèle "${info.name}" enregistré.`, modelId: info.id });
    } catch (e) {
      setResult({ ok: false, message: String(e) });
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex flex-1 flex-col items-center justify-center p-8">
      <div className="w-full max-w-md space-y-6">
        <div className="flex items-center gap-3">
          <Upload size={24} className="text-accent" />
          <h2 className="text-lg font-semibold text-fg">Importer un modèle GGUF</h2>
        </div>

        <p className="text-sm text-muted">
          Sélectionnez un fichier <code>.gguf</code> sur votre disque pour l'ajouter au catalogue.
          Le modèle sera utilisable via <strong>llama.cpp</strong> (provider built-in).
        </p>

        <div className="flex gap-2">
          <button
            onClick={() => void pickFile()}
            className="flex items-center gap-2 rounded border border-border bg-surface px-3 py-2 text-sm text-fg hover:bg-border/40"
          >
            <FolderOpen size={14} />
            Parcourir…
          </button>
          {path && (
            <span className="flex-1 truncate rounded border border-border bg-bg px-3 py-2 text-xs text-muted">
              {path}
            </span>
          )}
        </div>

        <button
          onClick={() => void handleImport()}
          disabled={!path.trim() || loading}
          className="w-full rounded bg-accent px-4 py-2 text-sm font-medium text-bg disabled:opacity-50 hover:opacity-90"
        >
          {loading ? "Importation…" : "Importer"}
        </button>

        {result && (
          <div
            className={`flex items-start gap-2 rounded border p-3 text-sm ${
              result.ok
                ? "border-green-600/40 bg-green-900/20 text-green-300"
                : "border-red-600/40 bg-red-900/20 text-red-300"
            }`}
          >
            {result.ok ? <CheckCircle size={16} className="mt-0.5 shrink-0" /> : <AlertCircle size={16} className="mt-0.5 shrink-0" />}
            <div>
              <p>{result.message}</p>
              {result.modelId && (
                <p className="mt-1 text-xs text-muted">ID : {result.modelId}</p>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
