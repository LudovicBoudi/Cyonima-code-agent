import { useSessionsStore } from "../store/sessions";

export function StatusBar() {
  const activeSessionId = useSessionsStore((s) => s.activeSessionId);
  const sessions = useSessionsStore((s) => s.sessions);

  const active = sessions.find((s) => s.id === activeSessionId);

  return (
    <footer className="flex h-6 items-center gap-3 border-t border-border px-3 text-xs text-muted">
      <span>Cyonima IA v1.1.2</span>
      <span>•</span>
      <span>{active ? `${active.modelId} · ${active.providerId}` : "Aucune session active"}</span>
      <span className="ml-auto">MIT • 100% local</span>
    </footer>
  );
}