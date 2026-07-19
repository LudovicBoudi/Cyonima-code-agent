import { create } from "zustand";
import type { SessionInfo, ChatMessage } from "../lib/ipc";
import { ipc } from "../lib/ipc";

type ProviderId = "llama_cpp" | "ollama" | "openai" | "anthropic" | "gemini" | "openai_compat";

export interface ToolCallItem {
  callId: string;
  tool: string;
  arguments: unknown;
  /// résultat quand disponible (`is_error` si échec)
  result?: { output: string; isError: boolean };
  /// Dénied par l'utilisateur (résultat: refus)
  denied?: boolean;
}

interface SessionsState {
  sessions: SessionInfo[];
  activeSessionId: string | null;
  messages: Record<string, ChatMessage[]>;
  /// Tool calls inline par session (affichés sous le dernier message assistant).
  toolCalls: Record<string, ToolCallItem[]>;
  streaming: Record<string, boolean>;
  errors: Record<string, string | null>;
  creating: boolean;

  startCreating: () => void;
  cancelCreating: () => void;
  createSession: (p: { workspace: string; modelId: string; providerId: ProviderId }) => Promise<void>;

  setActive: (id: string | null) => void;
  appendMessage: (sessionId: string, msg: ChatMessage) => void;
  appendToken: (sessionId: string, token: string) => void;
  setStreaming: (sessionId: string, streaming: boolean) => void;
  setError: (sessionId: string, error: string | null) => void;

  addToolCall: (sessionId: string, call: ToolCallItem) => void;
  setToolResult: (sessionId: string, callId: string, output: string, isError: boolean) => void;

  send: (sessionId: string, message: string) => Promise<void>;
  cancel: (sessionId: string) => Promise<void>;
}

export const useSessionsStore = create<SessionsState>((set, get) => ({
  sessions: [],
  activeSessionId: null,
  messages: {},
  toolCalls: {},
  streaming: {},
  errors: {},
  creating: false,

  startCreating: () => set({ creating: true }),
  cancelCreating: () => set({ creating: false }),

  createSession: async ({ workspace, modelId, providerId }) => {
    const info = await ipc.sessionCreate({ workspace, modelId, providerId });
    set((st) => ({
      sessions: [...st.sessions, info],
      activeSessionId: info.id,
      creating: false,
      messages: { ...st.messages, [info.id]: [] },
      toolCalls: { ...st.toolCalls, [info.id]: [] },
      streaming: { ...st.streaming, [info.id]: false },
      errors: { ...st.errors, [info.id]: null },
    }));
  },

  setActive: (id) => set({ activeSessionId: id, creating: false }),
  appendMessage: (sessionId, msg) =>
    set((st) => ({
      messages: {
        ...st.messages,
        [sessionId]: [...(st.messages[sessionId] ?? []), msg],
      },
    })),
  appendToken: (sessionId, token) =>
    set((st) => {
      const msgs = st.messages[sessionId] ?? [];
      const last = msgs[msgs.length - 1];
      const updated =
        !last || last.role !== "assistant"
          ? [...msgs, { role: "assistant" as const, content: token }]
          : [...msgs.slice(0, -1), { ...last, content: last.content + token }];
      return { messages: { ...st.messages, [sessionId]: updated } };
    }),
  setStreaming: (sessionId, streaming) =>
    set((st) => ({ streaming: { ...st.streaming, [sessionId]: streaming } })),
  setError: (sessionId, error) =>
    set((st) => ({ errors: { ...st.errors, [sessionId]: error } })),

  addToolCall: (sessionId, call) =>
    set((st) => ({
      toolCalls: {
        ...st.toolCalls,
        [sessionId]: [...(st.toolCalls[sessionId] ?? []), call],
      },
    })),
  setToolResult: (sessionId, callId, output, isError) =>
    set((st) => {
      const calls = st.toolCalls[sessionId] ?? [];
      const updated = calls.map((c) =>
        c.callId === callId ? { ...c, result: { output, isError } } : c,
      );
      return { toolCalls: { ...st.toolCalls, [sessionId]: updated } };
    }),

  send: async (sessionId, message) => {
    const st = get();
    if (st.streaming[sessionId]) return;
    // Nettoie les tool calls du tour précédent avant d'enregistrer le user msg.
    set((s) => ({
      errors: { ...s.errors, [sessionId]: null },
      streaming: { ...s.streaming, [sessionId]: true },
      toolCalls: { ...s.toolCalls, [sessionId]: [] },
      messages: {
        ...s.messages,
        [sessionId]: [...(s.messages[sessionId] ?? []), { role: "user" as const, content: message }],
      },
    }));
    try {
      await ipc.sessionSend({ sessionId, message });
    } catch (e) {
      set((s) => ({
        streaming: { ...s.streaming, [sessionId]: false },
        errors: { ...s.errors, [sessionId]: String(e) },
      }));
    }
  },

  cancel: async (sessionId) => {
    try {
      await ipc.sessionCancel({ sessionId });
    } catch {
      /* ignore */
    }
    set((s) => ({ streaming: { ...s.streaming, [sessionId]: false } }));
  },
}));