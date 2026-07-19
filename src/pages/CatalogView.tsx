import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ipc, type ModelInfo, type HardwareInfo } from "../lib/ipc";
import { useDownloadsStore } from "../store/downloads";

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

  const refresh = () => {
    ipc
      .modelCatalogList()
      .then((list) => setModels(Array.isArray(list) ? list : []))
      .catch(() => setModels([]));
    invoke<HardwareInfo>("hardware_get").then(setHw).catch(() => null);
  };

  useEffect(() => {
    refresh();
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
        <span>{filtered.length} / {models.length} modèles</span>
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
          onClick={refresh}
          className="ml-auto rounded border border-border px-2 py-1 text-xs text-muted hover:bg-border/40"
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
              <Row key={m.id} m={m} hw={hw} onInstalled={refresh} />
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function Row({ m, hw, onInstalled }: { m: ModelInfo; hw: HardwareInfo | null; onInstalled: () => void }) {
  const download = useDownloadsStore((s) => s.downloads[m.id]);
  const start = useDownloadsStore((s) => s.start);
  const cancel = useDownloadsStore((s) => s.cancel);

  const vramGb = hw?.vramGb ?? 0;
  const relaxed = vramGb > 0 && vramGb >= m.ramMinGb;
  const requiredRelaxed = relaxed ? Math.max(0, m.ramMinGb - 1) : m.ramMinGb + 1;
  const eligible = hw ? hw.totalRamGb >= requiredRelaxed : false;

  const isDownloading = !!download && !download.done && !download.error;
  const pct = download && download.total > 0 ? Math.round((download.downloaded / download.total) * 100) : 0;

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
          className="rounded border border-border px-2 py-1 text-xs text-muted hover:bg-border/40"
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
    if (!eligible) {
      return <span className="text-muted">Bloqué</span>;
    }
    return (
      <button
        onClick={() => start(m.id)}
        className="rounded bg-accent px-3 py-1 text-xs text-white hover:bg-accent/80"
      >
        Télécharger
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
          <td colSpan={8} className="px-4 py-2">
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
      {download?.error && (
        <tr className="border-t border-border/20 bg-red-500/5">
          <td colSpan={8} className="px-4 py-2 text-xs text-red-300">
            {download.error}
          </td>
        </tr>
      )}
    </>
  );
}