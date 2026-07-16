import { useEffect } from "react";
import { SessionsView } from "./pages/SessionsView";
import { Sidebar } from "./components/Sidebar";
import { StatusBar } from "./components/StatusBar";
import { useSessionsStore } from "./store/sessions";
import { onSessionToken, onSessionDone, onSessionError } from "./lib/ipc";

export default function App() {
  const appendToken = useSessionsStore((s) => s.appendToken);
  const setStreaming = useSessionsStore((s) => s.setStreaming);
  const setError = useSessionsStore((s) => s.setError);

  useEffect(() => {
    const unlistens: Array<() => void> = [];
    (async () => {
      unlistens.push(await onSessionToken((e) => appendToken(e.sessionId, e.token)));
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
  }, [appendToken, setStreaming, setError]);

  return (
    <div className="flex h-screen flex-col">
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />
        <main className="flex flex-1 flex-col overflow-hidden">
          <SessionsView />
        </main>
      </div>
      <StatusBar />
    </div>
  );
}