import { useEffect, useState } from "react";
import { SessionsView } from "./pages/SessionsView";
import { CatalogView } from "./pages/CatalogView";
import { Sidebar } from "./components/Sidebar";
import { StatusBar } from "./components/StatusBar";
import { PermissionDialog } from "./components/PermissionDialog";
import { useSessionsStore } from "./store/sessions";
import {
  onSessionToken,
  onSessionDone,
  onSessionError,
  onSessionToolCall,
  onSessionToolResult,
} from "./lib/ipc";

type View = "sessions" | "catalog";

export default function App() {
  const [view, setView] = useState<View>("sessions");
  const creating = useSessionsStore((s) => s.creating);
  const activeSessionId = useSessionsStore((s) => s.activeSessionId);

  const appendToken = useSessionsStore((s) => s.appendToken);
  const setStreaming = useSessionsStore((s) => s.setStreaming);
  const setError = useSessionsStore((s) => s.setError);
  const addToolCall = useSessionsStore((s) => s.addToolCall);
  const setToolResult = useSessionsStore((s) => s.setToolResult);

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
    })();
    return () => unlistens.forEach((u) => u());
  }, [appendToken, setStreaming, setError, addToolCall, setToolResult]);

  useEffect(() => {
    if (creating || activeSessionId) setView("sessions");
  }, [creating, activeSessionId]);

  return (
    <div className="flex h-screen flex-col">
      <div className="flex flex-1 overflow-hidden">
        <Sidebar view={view} onView={setView} />
        <main className="flex flex-1 flex-col overflow-hidden">
          {view === "sessions" ? <SessionsView /> : <CatalogView />}
        </main>
      </div>
      <StatusBar />
      <PermissionDialog />
    </div>
  );
}