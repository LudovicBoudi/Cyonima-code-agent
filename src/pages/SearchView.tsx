import { useEffect, useState } from "react";
import { ipc, type SearchResult, type IndexStats } from "../lib/ipc";

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

  const loadChunkCount = async () => {
    try {
      const count = await ipc.indexCount();
      setChunkCount(count);
    } catch {
      // Index pas encore créé.
    }
  };

  // Charger le nombre de chunks au montage.
  useEffect(() => {
    loadChunkCount();
  }, []);

  return (
    <div className="flex flex-col h-full p-4 gap-4">
      <h1 className="text-lg font-bold">Recherche sémantique</h1>

      {/* Indexation */}
      <div className="flex items-center gap-3">
        <button
          onClick={handleIndex}
          disabled={indexing}
          className="px-3 py-1.5 bg-blue-600 text-white rounded text-sm hover:bg-blue-700 disabled:opacity-50"
        >
          {indexing ? "Indexation..." : "Indexer le workspace"}
        </button>
        {chunkCount !== null && (
          <span className="text-xs text-neutral-500">
            {chunkCount} chunks indexés
          </span>
        )}
        {indexStats && (
          <span className="text-xs text-green-500">
            {indexStats.filesScanned} fichiers, {indexStats.chunksEmbedded} chunks
            {indexStats.errors.length > 0 &&
              ` (${indexStats.errors.length} erreurs)`}
          </span>
        )}
      </div>

      {/* Barre de recherche */}
      <div className="flex gap-2">
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleSearch()}
          placeholder="Ex: où est géré le panier dans le code"
          className="flex-1 px-3 py-2 bg-neutral-800 border border-neutral-700 rounded text-sm focus:outline-none focus:border-blue-500"
        />
        <button
          onClick={handleSearch}
          disabled={loading || !query.trim()}
          className="px-4 py-2 bg-green-600 text-white rounded text-sm hover:bg-green-700 disabled:opacity-50"
        >
          {loading ? "Recherche..." : "Chercher"}
        </button>
      </div>

      {/* Erreur */}
      {error && (
        <div className="p-3 bg-red-900/30 border border-red-700 rounded text-sm text-red-400">
          {error}
        </div>
      )}

      {/* Résultats */}
      <div className="flex-1 overflow-y-auto space-y-3">
        {results.length === 0 && !loading && query && !error && (
          <p className="text-sm text-neutral-500">Aucun résultat.</p>
        )}
        {results.map((r, i) => (
          <div
            key={i}
            className="p-3 bg-neutral-800 border border-neutral-700 rounded"
          >
            <div className="flex items-center justify-between mb-1">
              <span className="text-xs font-mono text-blue-400">
                {r.filePath}
              </span>
              <span className="text-xs text-neutral-500">
                lignes {r.startLine}-{r.endLine} &middot; score{" "}
                {(r.score * 100).toFixed(1)}%
              </span>
            </div>
            <pre className="text-xs text-neutral-300 whitespace-pre-wrap overflow-x-auto max-h-40 overflow-y-auto">
              {r.text}
            </pre>
          </div>
        ))}
      </div>
    </div>
  );
}
