import { create } from "zustand";
import type { SessionInfo, ChatMessage } from "../lib/ipc";
import { ipc } from "../lib/ipc";

type ProviderId = "llama_cpp" | "ollama" | "openai" | "anthropic" | "gemini" | "openai_compat";

interface SessionsState {
  sessions: SessionInfo[];
  activeSessionId: string | null;
  messages: Record<string, ChatMessage[]>;
  streaming: Record<string, boolean>;
  errors: Record<string, string | null>;
  creating: boolean;

  startCreating: () => void;
  cancelCreating: () => void;
  createSession: (p: {
    workspace: string;
    modelId: string;
    providerId: ProviderId;
  }) => Promise<void>;

  setActive: (id: string | null) => void;
  appendMessage: (sessionId: string, msg: ChatMessage) => void;
  appendToken: (sessionId: string, token: string) => void;
  setStreaming: (sessionId: string, streaming: boolean) => void;
  setError: (sessionId: string, error: string | null) => void;

  send: (sessionId: string, message: string) => Promise<void>;
  cancel: (sessionId: string) => Promise<void>;
}

export const useSessionsStore = create<SessionsState>((set, get) => ({
  sessions: [],
  activeSessionId: null,
  messages: {},
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
      const updated =
        msgs.length === 0 || msgs[msgs.length - 1].role !== "assistant"
          ? [...msgs, { role: "assistant" as const, content: token }]
          : [...msgs.slice(0, -1), { ...msgs[msgs.length - 1], content: msgs[msgs.length - 1].content + token }];
      return { messages: { ...st.messages, [sessionId]: updated } };
    }),
  setStreaming: (sessionId, streaming) =>
    set((st) => ({ streaming: { ...st.streaming, [sessionId]: streaming } })),
  setError: (sessionId, error) =>
    set((st) => ({ errors: { ...st.errors, [sessionId]: error } })),

  send: async (sessionId, message) => {
    const st = get();
    if (st.streaming[sessionId]) return;
    set((s) => ({
      errors: { ...s.errors, [sessionId]: null },
      streaming: { ...s.streaming, [sessionId]: true },
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