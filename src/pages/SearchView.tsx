import { useEffect, useState } from "react";
import { ipc, type SearchResult, type IndexStats } from "../lib/ipc";
import { Search, Play, Loader2 } from "lucide-react";

export default function SearchView() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [indexStats, setIndexStats] = useState<IndexStats | null>(null);
  const [indexing, setIndexing] = useState(false);
  const [chunkCount, setChunkCount] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleSearch = async () => {
    if (!query.trim()) return;
    setLoading(true);
    setError(null);
    try {
      const res = await ipc.indexSearch({
        workspace: ".",
        query: query.trim(),
        limit: 10,
      });
      setResults(res);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleIndex = async () => {
    setIndexing(true);
    setError(null);
    try {
      const stats = await ipc.indexBuild({ workspace: "." });
      setIndexStats(stats);
      const count = await ipc.indexCount();
      setChunkCount(count);
    } catch (e) {
      setError(String(e));
    } finally {
      setIndexing(false);
    }
  };

  useEffect(() => {
    void (async () => {
      try {
        const count = await ipc.indexCount();
        setChunkCount(count);
      } catch {
        // Index pas encore créé.
      }
    })();
  }, []);

  return (
    <div className="flex flex-col h-full p-4 gap-4">
      <h1 className="text-lg font-bold">Recherche sémantique</h1>

      <div className="flex items-center gap-3">
        <button
          onClick={handleIndex}
          disabled={indexing}
          className="flex items-center gap-1.5 px-3 py-1.5 bg-accent text-white rounded text-sm hover:bg-accent/80 disabled:opacity-50"
        >
          {indexing ? <Loader2 size={14} className="animate-spin" /> : <Play size={14} />}
          {indexing ? "Indexation..." : "Indexer le workspace"}
        </button>
        {chunkCount !== null && (
          <span className="text-xs text-muted">{chunkCount} chunks indexés</span>
        )}
        {indexStats && (
          <span className="text-xs text-green-400">
            {indexStats.filesScanned} fichiers, {indexStats.chunksEmbedded} chunks
            {indexStats.errors.length > 0 && ` (${indexStats.errors.length} erreurs)`}
          </span>
        )}
      </div>

      <div className="flex gap-2">
        <div className="relative flex-1">
          <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-muted" />
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleSearch()}
            placeholder="Ex: où est géré le panier dans le code"
            className="w-full pl-8 pr-3 py-2 bg-surface border border-border rounded text-sm focus:outline-none focus:border-accent"
          />
        </div>
        <button
          onClick={handleSearch}
          disabled={loading || !query.trim()}
          className="px-4 py-2 bg-accent text-white rounded text-sm hover:bg-accent/80 disabled:opacity-50"
        >
          {loading ? "Recherche..." : "Chercher"}
        </button>
      </div>

      {error && (
        <div className="p-3 bg-red-500/10 border border-red-500/40 rounded text-sm text-red-400">
          {error}
        </div>
      )}

      <div className="flex-1 overflow-y-auto space-y-3">
        {results.length === 0 && !loading && query && !error && (
          <p className="text-sm text-muted">Aucun résultat.</p>
        )}
        {results.map((r, i) => (
          <div key={i} className="p-3 bg-surface border border-border rounded">
            <div className="flex items-center justify-between mb-1">
              <span className="text-xs font-mono text-accent">{r.filePath}</span>
              <span className="text-xs text-muted">
                lignes {r.startLine}-{r.endLine} &middot; score{" "}
                {(r.score * 100).toFixed(1)}%
              </span>
            </div>
            <pre className="text-xs text-fg whitespace-pre-wrap overflow-x-auto max-h-40 overflow-y-auto font-mono">
              {r.text}
            </pre>
          </div>
        ))}
      </div>
    </div>
  );
}
