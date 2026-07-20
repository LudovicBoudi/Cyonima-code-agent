import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ipc, type ModelInfo, type HardwareInfo } from "../lib/ipc";
import { useDownloadsStore } from "../store/downloads";
import { Download } from "lucide-react";

/// Progression d'un pull Ollama, dérivée du flux JSON de `/api/pull`.
type OllamaPullProgress = {
  status: string;
  completed: number;
  total: number;
};

function formatSize(bytes: number): string {
  if (bytes === 0) return "—";
  if (bytes < 1 << 30) return `${(bytes / (1 << 20)).toFixed(0)} Mo`;
  return `${(bytes / (1 << 30)).toFixed(1)} Go`;
}

function formatBps(bps: number): string {
  if (bps === 0) return "";
  if (bps < 1 << 10) return `${bps} o/s`;
  if (bps < 1 << 20) return `${(bps / (1 << 10)).toFixed(1)} Ko/s`;
  return `${(bps / (1 << 20)).toFixed(2)} Mo/s`;
}

export function CatalogView() {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [hw, setHw] = useState<HardwareInfo | null>(null);
  const [filter, setFilter] = useState("");
  const [ollamaPulling, setOllamaPulling] = useState<string | null>(null);
  const [pullProgress, setPullProgress] = useState<OllamaPullProgress | null>(null);

  const refresh = () => {
    ipc
      .modelCatalogList()
      .then((list) => setModels(Array.isArray(list) ? list : []))
      .catch(() => setModels([]));
    invoke<HardwareInfo>("hardware_get").then(setHw).catch(() => null);
  };

  const handleOllamaPull = async (ollamaTag: string) => {
    try {
      setOllamaPulling(ollamaTag);
      setPullProgress(null);
      await invoke("ollama_pull_model", { model: ollamaTag });
    } catch (error) {
      console.error("Erreur pull Ollama:", error);
      setOllamaPulling(null);
      setPullProgress(null);
    }
  };

  const handleClearCache = async () => {
    if (!confirm("Voulez-vous vraiment vider le cache des modèles téléchargés ? Cette action est irréversible.")) {
      return;
    }
    try {
      await ipc.modelClearCache();
      refresh(); // Rafraîchir la liste après nettoyage
      alert("Cache vidé avec succès !");
    } catch (error) {
      console.error("Erreur nettoyage cache:", error);
      alert("Erreur lors du nettoyage : " + error);
    }
  };

  useEffect(() => {
    refresh();
    
    // Écouter les événements Ollama
    const unlistenPullProgress = listen<OllamaPullProgress>("ollama:pull:progress", (event) => {
      const p = event.payload;
      setPullProgress({
        status: p.status ?? "",
        completed: p.completed ?? 0,
        total: p.total ?? 0,
      });
    });
    const unlistenPullDone = listen("ollama:pull:done", () => {
      setOllamaPulling(null);
      setPullProgress(null);
      refresh(); // Rafraîchir après succès
    });
    const unlistenPullError = listen("ollama:pull:error", () => {
      setOllamaPulling(null);
      setPullProgress(null);
    });

    return () => {
      unlistenPullProgress.then(f => f());
      unlistenPullDone.then(f => f());
      unlistenPullError.then(f => f());
    };
  }, []);

  const lower = filter.trim().toLowerCase();
  const filtered = lower
    ? models.filter(
        (m) =>
          m.name.toLowerCase().includes(lower) ||
          m.id.toLowerCase().includes(lower) ||
          m.license.toLowerCase().includes(lower),
      )
    : models;

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      <header className="flex items-center gap-3 border-b border-border px-4 py-2 text-xs text-muted">
        <span className="font-semibold text-fg">Catalogue de modèles</span>
        <span>·</span>
        <span>{filtered.length} / {models.length} modèles (Ollama uniquement)</span>
        {hw && (
          <>
            <span>·</span>
            <span>
              {hw.totalRamGb} Go RAM
              {hw.vramGb > 0 && ` + ${hw.vramGb} Go VRAM`}
            </span>
          </>
        )}
        <button
          onClick={handleClearCache}
          className="rounded border border-red-500/40 px-2 py-1 text-xs text-red-300 hover:bg-red-500/10 mr-2"
        >
          Vider cache GGUF
        </button>
        <button
          onClick={refresh}
          className="rounded border border-border px-2 py-1 text-xs text-muted hover:bg-border/40"
        >
          Rafraîchir
        </button>
        <input
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          placeholder="Filtrer (nom, id, licence)…"
          className="w-64 rounded border border-border bg-bg px-2 py-1 text-xs"
        />
      </header>

      <div className="flex-1 overflow-y-auto">
        <table className="w-full text-left text-xs">
          <thead className="sticky top-0 bg-bg text-muted">
            <tr>
              <th className="px-4 py-2 font-medium">Nom</th>
              <th className="px-2 py-2 font-medium">Type</th>
              <th className="px-2 py-2 font-medium">Quant.</th>
              <th className="px-2 py-2 font-medium">Taille</th>
              <th className="px-2 py-2 font-medium">RAM min</th>
              <th className="px-2 py-2 font-medium">Licence</th>
              <th className="px-2 py-2 font-medium">Ollama</th>
              <th className="px-4 py-2 font-medium text-right">Éligibilité</th>
              <th className="px-4 py-2 font-medium text-right">Action</th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((m) => (
              <Row key={m.id} m={m} hw={hw} onInstalled={refresh} onOllamaPull={handleOllamaPull} ollamaPulling={ollamaPulling} pullProgress={pullProgress} isAnyPulling={ollamaPulling !== null} />
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function Row({ 
  m, 
  hw, 
  onInstalled, 
  onOllamaPull, 
  ollamaPulling,
  pullProgress,
  isAnyPulling,
}: { 
  m: ModelInfo; 
  hw: HardwareInfo | null; 
  onInstalled: () => void;
  onOllamaPull: (ollamaTag: string) => void;
  ollamaPulling: string | null;
  pullProgress: OllamaPullProgress | null;
  isAnyPulling: boolean;
}) {
  const download = useDownloadsStore((s) => s.downloads[m.id]);
  const start = useDownloadsStore((s) => s.start);
  const cancel = useDownloadsStore((s) => s.cancel);

  const vramGb = hw?.vramGb ?? 0;
  const relaxed = vramGb > 0 && vramGb >= m.ramMinGb;
  const requiredRelaxed = relaxed ? Math.max(0, m.ramMinGb - 1) : m.ramMinGb + 1;
  const eligible = hw ? hw.totalRamGb >= requiredRelaxed : false;

  const isDownloading = !!download && !download.done && !download.error;
  const isOllamaPulling = ollamaPulling === m.ollamaTag;
  // Un pull Ollama est en cours sur une AUTRE ligne : on bloque les
  // boutons de téléchargement pour éviter les états d'affichage incohérents.
  const blockedByOtherPull = isAnyPulling && !isOllamaPulling;
  const pct = download && download.total > 0 ? Math.round((download.downloaded / download.total) * 100) : 0;
  const ollamaPct =
    pullProgress && pullProgress.total > 0
      ? Math.round((pullProgress.completed / pullProgress.total) * 100)
      : 0;

  const renderStatus = () => {
    if (m.installed) {
      return <span className="rounded bg-green-500/10 px-2 py-0.5 text-green-400">Installé</span>;
    }
    if (download?.error) {
      return (
        <span className="rounded bg-red-500/10 px-2 py-0.5 text-red-400" title={download.error}>
          Erreur
        </span>
      );
    }
    if (download?.done) {
      return <span className="rounded bg-green-500/10 px-2 py-0.5 text-green-400">Téléchargé</span>;
    }
    if (!hw) {
      return <span className="text-muted">?</span>;
    }
    return eligible ? (
      <span className="rounded bg-accent/10 px-2 py-0.5 text-accent">OK</span>
    ) : (
      <span className="rounded bg-red-500/10 px-2 py-0.5 text-red-400">RAM insuff.</span>
    );
  };

  const renderAction = () => {
    if (m.installed) {
      return <span className="text-muted">—</span>;
    }
    if (download?.error) {
      return (
        <button
          onClick={() => start(m.id)}
          disabled={blockedByOtherPull}
          className="rounded border border-border px-2 py-1 text-xs text-muted hover:bg-border/40 disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent"
        >
          Réessayer
        </button>
      );
    }
    if (download?.done) {
      return (
        <button
          onClick={onInstalled}
          className="rounded border border-border px-2 py-1 text-xs text-muted hover:bg-border/40"
        >
          Rafraîchir
        </button>
      );
    }
    if (isDownloading) {
      return (
        <button
          onClick={() => cancel(m.id)}
          className="rounded border border-red-500/40 px-2 py-1 text-xs text-red-300 hover:bg-red-500/10"
        >
          Annuler
        </button>
      );
    }
    if (isOllamaPulling) {
      return (
        <span className="flex items-center gap-1.5 rounded border border-blue-500/40 px-2 py-1 text-xs text-blue-300">
          <Download size={12} className="animate-pulse" />
          {ollamaPct > 0 ? `${ollamaPct}%` : "Pull..."}
        </span>
      );
    }
    if (!eligible) {
      return <span className="text-muted">Bloqué</span>;
    }
    
    // Privilégier Ollama si un tag existe
    if (m.ollamaTag) {
      return (
        <button
          onClick={() => onOllamaPull(m.ollamaTag!)}
          disabled={isOllamaPulling || blockedByOtherPull}
          title={blockedByOtherPull ? "Un téléchargement Ollama est déjà en cours" : undefined}
          className="flex items-center gap-1.5 rounded bg-blue-600 px-3 py-1 text-xs text-white hover:bg-blue-500 disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-blue-600"
        >
          <Download size={12} />
          {isOllamaPulling ? "Pull..." : "Pull Ollama"}
        </button>
      );
    }
    
    return (
      <button
        onClick={() => start(m.id)}
        disabled={blockedByOtherPull}
        title={blockedByOtherPull ? "Un téléchargement Ollama est déjà en cours" : undefined}
        className="rounded bg-accent px-3 py-1 text-xs text-white hover:bg-accent/80 disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-accent"
      >
        Télécharger GGUF
      </button>
    );
  };

  return (
    <>
      <tr className="border-t border-border/60 hover:bg-border/20">
        <td className="px-4 py-2">
          <div className="font-medium text-fg">{m.name}</div>
          <div className="text-muted">{m.id}</div>
        </td>
        <td className="px-2 py-2">
          {m.modelType === "coding" ? (
            <span className="rounded bg-blue-500/10 px-2 py-0.5 text-blue-400 text-xs">Code</span>
          ) : (
            <span className="rounded bg-green-500/10 px-2 py-0.5 text-green-400 text-xs">Général</span>
          )}
        </td>
        <td className="px-2 py-2 text-muted">{m.quantization}</td>
        <td className="px-2 py-2 text-muted">{formatSize(m.sizeBytes)}</td>
        <td className="px-2 py-2 text-muted">{m.ramMinGb} Go</td>
        <td className="px-2 py-2 text-muted">{m.license}</td>
        <td className="px-2 py-2">
          {m.ollamaTag ? (
            <code className="rounded bg-border/40 px-1.5 py-0.5 text-accent">{m.ollamaTag}</code>
          ) : (
            <span className="text-muted">—</span>
          )}
        </td>
        <td className="px-4 py-2 text-right">{renderStatus()}</td>
        <td className="px-4 py-2 text-right">{renderAction()}</td>
      </tr>
      {isDownloading && (
        <tr className="border-t border-border/20 bg-accent/5">
          <td colSpan={9} className="px-4 py-2">
            <div className="flex items-center gap-2 text-xs">
              <div className="h-2 flex-1 overflow-hidden rounded bg-border/40">
                <div
                  className="h-full bg-accent transition-all"
                  style={{ width: `${pct}%` }}
                />
              </div>
              <span className="w-12 text-right tabular-nums text-fg">
                {pct}%
              </span>
              <span className="w-32 text-right text-muted tabular-nums">
                {formatSize(download?.downloaded ?? 0)} / {formatSize(download?.total ?? 0)}
              </span>
              <span className="w-24 text-right text-muted tabular-nums">
                {formatBps(download?.bytesPerSecond ?? 0)}
              </span>
            </div>
          </td>
        </tr>
      )}
      {isOllamaPulling && (
        <tr className="border-t border-border/20 bg-blue-500/5">
          <td colSpan={9} className="px-4 py-2">
            <div className="flex items-center gap-2 text-xs">
              <div className="h-2 flex-1 overflow-hidden rounded bg-border/40">
                <div
                  className={`h-full bg-blue-500 transition-all ${ollamaPct === 0 ? "animate-pulse" : ""}`}
                  style={{ width: ollamaPct > 0 ? `${ollamaPct}%` : "100%" }}
                />
              </div>
              <span className="w-12 text-right tabular-nums text-fg">
                {ollamaPct > 0 ? `${ollamaPct}%` : ""}
              </span>
              <span className="w-32 text-right text-muted tabular-nums">
                {pullProgress && pullProgress.total > 0
                  ? `${formatSize(pullProgress.completed)} / ${formatSize(pullProgress.total)}`
                  : ""}
              </span>
              <span className="w-40 truncate text-right text-muted">
                {pullProgress?.status ?? "Connexion à Ollama…"}
              </span>
            </div>
          </td>
        </tr>
      )}
      {download?.error && (
        <tr className="border-t border-border/20 bg-red-500/5">
          <td colSpan={9} className="px-4 py-2 text-xs text-red-300">
            {download.error}
          </td>
        </tr>
      )}
    </>
  );
}