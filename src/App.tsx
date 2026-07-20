import { useEffect, useState } from "react";
import { SessionsView } from "./pages/SessionsView";
import { CatalogView } from "./pages/CatalogView";
import { ImportModelView } from "./pages/ImportModelView";
import { SettingsView } from "./pages/SettingsView";
import { Sidebar } from "./components/Sidebar";
import { StatusBar } from "./components/StatusBar";
import { PermissionDialog } from "./components/PermissionDialog";
import { useSessionsStore } from "./store/sessions";
import { useDownloadsStore } from "./store/downloads";
import {
  onSessionToken,
  onSessionDone,
  onSessionError,
  onSessionToolCall,
  onSessionToolResult,
  onDownloadProgress,
  onDownloadDone,
  onDownloadError,
} from "./lib/ipc";

type View = "sessions" | "catalog" | "import" | "settings";

export default function App() {
  const [view, setView] = useState<View>("sessions");
  const creating = useSessionsStore((s) => s.creating);
  const activeSessionId = useSessionsStore((s) => s.activeSessionId);
  const loaded = useSessionsStore((s) => s.loaded);
  const loadAll = useSessionsStore((s) => s.loadAll);

  const appendToken = useSessionsStore((s) => s.appendToken);
  const setStreaming = useSessionsStore((s) => s.setStreaming);
  const setError = useSessionsStore((s) => s.setError);
  const addToolCall = useSessionsStore((s) => s.addToolCall);
  const setToolResult = useSessionsStore((s) => s.setToolResult);
  const setProgress = useDownloadsStore((s) => s.setProgress);
  const markDownloadDone = useDownloadsStore((s) => s.markDone);
  const markDownloadError = useDownloadsStore((s) => s.markError);

  // Au démarrage : recharge les sessions persistées en SQLite.
  useEffect(() => {
    if (!loaded) {
      void loadAll();
    }
  }, [loaded, loadAll]);

  useEffect(() => {
    const unlistens: Array<() => void> = [];
    (async () => {
      unlistens.push(await onSessionToken((e) => appendToken(e.sessionId, e.token)));
      unlistens.push(
        await onSessionToolCall((e) =>
          addToolCall(e.sessionId, { callId: e.callId, tool: e.tool, arguments: e.arguments }),
        ),
      );
      unlistens.push(
        await onSessionToolResult((e) => setToolResult(e.sessionId, e.callId, e.output, e.isError)),
      );
      unlistens.push(
        await onSessionDone((e) => {
          setStreaming(e.sessionId, false);
        }),
      );
      unlistens.push(
        await onSessionError((e) => {
          setError(e.sessionId, e.error);
          setStreaming(e.sessionId, false);
        }),
      );
      unlistens.push(
        await onDownloadProgress((e) =>
          setProgress({
            modelId: e.modelId,
            downloaded: e.downloaded,
            total: e.total,
            bytesPerSecond: e.bytesPerSecond,
          }),
        ),
      );
      unlistens.push(
        await onDownloadDone((e) => {
          markDownloadDone(e.modelId);
        }),
      );
      unlistens.push(
        await onDownloadError((e) => {
          markDownloadError(e.modelId, e.error);
        }),
      );
    })();
    return () => unlistens.forEach((u) => u());
  }, [
    appendToken,
    setStreaming,
    setError,
    addToolCall,
    setToolResult,
    setProgress,
    markDownloadDone,
    markDownloadError,
  ]);

  useEffect(() => {
    if (creating || activeSessionId) setView("sessions");
  }, [creating, activeSessionId]);

  return (
    <div className="flex h-screen flex-col">
      <div className="flex flex-1 overflow-hidden">
        <Sidebar view={view} onView={setView} />
        <main className="flex flex-1 flex-col overflow-hidden">
          {view === "sessions" && <SessionsView />}
          {view === "catalog" && <CatalogView />}
          {view === "import" && <ImportModelView />}
          {view === "settings" && <SettingsView />}
        </main>
      </div>
      <StatusBar />
      <PermissionDialog />
    </div>
  );
}