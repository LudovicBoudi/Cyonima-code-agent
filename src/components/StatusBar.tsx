import { useSessionsStore } from "../store/sessions";
import { useDownloadsStore } from "../store/downloads";

export function StatusBar() {
  const activeSessionId = useSessionsStore((s) => s.activeSessionId);
  const sessions = useSessionsStore((s) => s.sessions);
  const downloads = useDownloadsStore((s) => s.downloads);

  const active = sessions.find((s) => s.id === activeSessionId);
  const activeDownloads = Object.values(downloads).filter((d) => !d.done && !d.error);

  return (
    <footer className="flex h-6 items-center gap-3 border-t border-border px-3 text-xs text-muted">
      <span>Cyonima IA v0.1.0</span>
      <span>•</span>
      <span>{active ? `${active.modelId} · ${active.providerId}` : "Aucune session active"}</span>
      {activeDownloads.length > 0 && (
        <>
          <span>•</span>
          <span className="text-accent">
            {activeDownloads.length} téléchargement{activeDownloads.length > 1 ? "s" : ""} en cours
          </span>
        </>
      )}
      <span className="ml-auto">MIT • 100% local</span>
    </footer>
  );
}