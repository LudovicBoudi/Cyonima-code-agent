import { useEffect, useRef, useState } from "react";
import { SessionsView } from "./pages/SessionsView";
import { CatalogView } from "./pages/CatalogView";
import { ImportModelView } from "./pages/ImportModelView";
import { SettingsView } from "./pages/SettingsView";
import { OllamaView } from "./pages/OllamaView";
import { ConfigView } from "./pages/ConfigView";
import SearchView from "./pages/SearchView";
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
import { useKeyboardShortcuts, registerShortcut } from "./lib/shortcuts";

type View = "sessions" | "catalog" | "import" | "settings" | "ollama" | "config" | "search";

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

  // Raccourcis clavier
  const viewRef = useRef(view);
  viewRef.current = view;
  const setViewRef = useRef(setView);
  setViewRef.current = setView;
  const startCreatingRef = useRef(useSessionsStore.getState().startCreating);
  startCreatingRef.current = useSessionsStore.getState().startCreating;
  const cancelRef = useRef(useSessionsStore.getState().cancel);
  cancelRef.current = useSessionsStore.getState().cancel;

  useEffect(() => {
    registerShortcut("n", () => {
      setViewRef.current("sessions");
      startCreatingRef.current();
    }, { ctrl: true });

    registerShortcut("f", () => {
      setViewRef.current("search");
    }, { ctrl: true });

    registerShortcut("Escape", () => {
      const state = useSessionsStore.getState();
      if (state.activeSessionId && state.streaming[state.activeSessionId]) {
        void state.cancel(state.activeSessionId);
      }
    });
  }, []);

  useKeyboardShortcuts();

  return (
    <div className="flex h-screen flex-col">
      <div className="flex flex-1 overflow-hidden">
        <Sidebar view={view} onView={setView} />
        <main className="flex flex-1 flex-col overflow-hidden">
          {view === "sessions" && <SessionsView />}
          {view === "catalog" && <CatalogView />}
          {view === "ollama" && <OllamaView />}
          {view === "import" && <ImportModelView />}
          {view === "settings" && <SettingsView />}
          {view === "config" && <ConfigView />}
          {view === "search" && <SearchView />}
        </main>
      </div>
      <StatusBar />
      <PermissionDialog />
    </div>
  );
}