import { create } from "zustand";
import type { SessionInfo, ChatMessage, OllamaModelInfo } from "../lib/ipc";
import { ipc } from "../lib/ipc";

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
  loaded: boolean;
  /// Contenu thinking en cours de réception par session.
  thinking: Record<string, string>;
  /// État de chargement des modèles par session (pour les providers locaux).
  modelLoading: Record<string, boolean>;
  /// Progression du chargement des modèles (0-100).
  modelLoadingProgress: Record<string, number>;
  /// Modèle Ollama sélectionné par session (menu déroulant du chat).
  selectedModels: Record<string, string>;
  /// Intensité de raisonnement par session ("auto"/"off"/"low"/"medium"/"high").
  reasoningLevels: Record<string, string>;
  /// Dernier usage de tokens rapporté par le backend (par session).
  lastUsage: Record<string, { tokensIn: number; tokensOut: number }>;
  /// Taille de contexte (tokens) connue par nom de modèle Ollama.
  modelContextLengths: Record<string, number | null>;
  /// Liste des modèles installés dans Ollama (source du menu déroulant).
  installedOllamaModels: OllamaModelInfo[];

  /// Au démarrage : charge les sessions persistées et, pour la dernière
  /// active (la plus récente), restaure aussi ses messages.
  loadAll: () => Promise<void>;

  /// Restaure les messages d'une session depuis le backend SQLite.
  restoreMessages: (sessionId: string) => Promise<void>;

  startCreating: () => void;
  cancelCreating: () => void;
  createSession: (p: { workspace: string }) => Promise<void>;

  /// Charge la liste des modèles installés dans Ollama.
  loadInstalledOllamaModels: () => Promise<void>;
  /// Sélectionne le modèle courant d'une session.
  setSelectedModel: (sessionId: string, model: string) => void;
  /// Règle l'intensité de raisonnement d'une session.
  setReasoning: (sessionId: string, level: string) => void;
  /// Enregistre l'usage de tokens (depuis session:done).
  setUsage: (sessionId: string, usage: { tokensIn: number; tokensOut: number }) => void;
  /// Charge la taille de contexte d'un modèle Ollama (mis en cache).
  loadModelContext: (model: string) => Promise<void>;

  setActive: (id: string | null) => void;
  deleteSession: (sessionId: string) => Promise<void>;
  appendMessage: (sessionId: string, msg: ChatMessage) => void;
  appendToken: (sessionId: string, token: string) => void;
  appendThinking: (sessionId: string, token: string) => void;
  clearThinking: (sessionId: string) => void;
  setStreaming: (sessionId: string, streaming: boolean) => void;
  setError: (sessionId: string, error: string | null) => void;
  setModelLoading: (sessionId: string, loading: boolean) => void;
  setModelLoadingProgress: (sessionId: string, progress: number) => void;
  markModelReady: (sessionId: string) => void;

  addToolCall: (sessionId: string, call: ToolCallItem) => void;
  setToolResult: (sessionId: string, callId: string, output: string, isError: boolean) => void;

  send: (sessionId: string, message: string) => Promise<void>;
  cancel: (sessionId: string) => Promise<void>;
  forkSession: (sessionId: string) => Promise<void>;
}

export const useSessionsStore = create<SessionsState>((set, get) => ({
  sessions: [],
  activeSessionId: null,
  messages: {},
  toolCalls: {},
  streaming: {},
  errors: {},
  creating: false,
  loaded: false,
  thinking: {},
  modelLoading: {},
  modelLoadingProgress: {},
  selectedModels: {},
  reasoningLevels: {},
  lastUsage: {},
  modelContextLengths: {},
  installedOllamaModels: [],

  loadInstalledOllamaModels: async () => {
    try {
      const models = await ipc.ollamaListModels();
      set({ installedOllamaModels: Array.isArray(models) ? models : [] });
    } catch (e) {
      console.error("ollamaListModels error", e);
      set({ installedOllamaModels: [] });
    }
  },

  setSelectedModel: (sessionId, model) =>
    set((st) => ({
      selectedModels: { ...st.selectedModels, [sessionId]: model },
    })),

  setReasoning: (sessionId, level) =>
    set((st) => ({
      reasoningLevels: { ...st.reasoningLevels, [sessionId]: level },
    })),

  setUsage: (sessionId, usage) =>
    set((st) => ({ lastUsage: { ...st.lastUsage, [sessionId]: usage } })),

  loadModelContext: async (model) => {
    if (!model || get().modelContextLengths[model] !== undefined) return;
    try {
      const info = await ipc.ollamaModelInfo({ model });
      set((st) => ({
        modelContextLengths: { ...st.modelContextLengths, [model]: info.contextLength },
      }));
    } catch {
      set((st) => ({
        modelContextLengths: { ...st.modelContextLengths, [model]: null },
      }));
    }
  },

  loadAll: async () => {
    try {
      const list = await ipc.sessionList();
      // Initialise les vues vides pour chaque session restaurée.
      const messages: Record<string, ChatMessage[]> = {};
      const toolCalls: Record<string, ToolCallItem[]> = {};
      const streaming: Record<string, boolean> = {};
      const errors: Record<string, string | null> = {};
      const thinking: Record<string, string> = {};
      const modelLoading: Record<string, boolean> = {};
      const modelLoadingProgress: Record<string, number> = {};
      for (const s of list) {
        messages[s.id] = [];
        toolCalls[s.id] = [];
        streaming[s.id] = false;
        errors[s.id] = null;
        thinking[s.id] = "";
        modelLoading[s.id] = false;
        modelLoadingProgress[s.id] = 0;
      }
      const activeId = list.length > 0 ? list[0].id : null;
      
      // Pour la session active, si c'est LlamaCpp, marquer comme en chargement
      if (activeId) {
        const activeSession = list.find(s => s.id === activeId);
        if (activeSession?.providerId === "llama_cpp") {
          modelLoading[activeId] = true;
          modelLoadingProgress[activeId] = 0;
        }
      }
      
      set({ 
        sessions: list, 
        activeSessionId: activeId, 
        messages, 
        toolCalls, 
        streaming, 
        errors, 
        thinking, 
        modelLoading,
        modelLoadingProgress,
        loaded: true 
      });
      // Restaure aussi les messages de la plus récente.
      if (activeId) {
        await get().restoreMessages(activeId);
      }
    } catch (e) {
      console.error("sessionList error", e);
      set({ loaded: true });
    }
  },

  restoreMessages: async (sessionId) => {
    try {
      const msgs = await ipc.sessionHistory({ sessionId });
      set((st) => ({ messages: { ...st.messages, [sessionId]: msgs ?? [] } }));
    } catch (e) {
      console.error("sessionHistory error", e);
    }
  },

  startCreating: () => set({ creating: true }),
  cancelCreating: () => set({ creating: false }),

  createSession: async ({ workspace }) => {
    // Provider Ollama unique ; le modèle est choisi ensuite via le menu
    // déroulant du chat (créé vide côté backend).
    const info = await ipc.sessionCreate({ workspace, modelId: "", providerId: "ollama" });
    // Pré-sélectionne le premier modèle Ollama installé, s'il y en a.
    const firstModel = get().installedOllamaModels[0]?.name ?? "";
    set((st) => ({
      sessions: [...st.sessions, info],
      activeSessionId: info.id,
      creating: false,
      messages: { ...st.messages, [info.id]: [] },
      toolCalls: { ...st.toolCalls, [info.id]: [] },
      streaming: { ...st.streaming, [info.id]: false },
      errors: { ...st.errors, [info.id]: null },
      thinking: { ...st.thinking, [info.id]: "" },
      modelLoading: { ...st.modelLoading, [info.id]: false },
      modelLoadingProgress: { ...st.modelLoadingProgress, [info.id]: 0 },
      selectedModels: { ...st.selectedModels, [info.id]: firstModel },
      reasoningLevels: { ...st.reasoningLevels, [info.id]: "auto" },
    }));
  },

  setActive: (id) => {
    set((st) => {
      const newState = { activeSessionId: id, creating: false };
      
      // Si la nouvelle session active utilise LlamaCpp, marquer comme en chargement
      if (id) {
        const session = st.sessions.find(s => s.id === id);
        if (session?.providerId === "llama_cpp") {
          return {
            ...newState,
            modelLoading: { ...st.modelLoading, [id]: true },
            modelLoadingProgress: { ...st.modelLoadingProgress, [id]: 0 },
          };
        }
      }
      
      return newState;
    });
  },

  deleteSession: async (sessionId) => {
    try {
      await ipc.sessionDelete({ sessionId });
    } catch {
      /* ignore */
    }
    set((st) => {
      const sessions = st.sessions.filter((s) => s.id !== sessionId);
      const messages = { ...st.messages };
      delete messages[sessionId];
      const toolCalls = { ...st.toolCalls };
      delete toolCalls[sessionId];
      const streaming = { ...st.streaming };
      delete streaming[sessionId];
      const errors = { ...st.errors };
      delete errors[sessionId];
      const activeSessionId =
        st.activeSessionId === sessionId
          ? sessions.length > 0
            ? sessions[0].id
            : null
          : st.activeSessionId;
      // Si la nouvelle active n'a pas encore ses messages, on les charge.
      if (activeSessionId && activeSessionId !== st.activeSessionId) {
        void get().restoreMessages(activeSessionId);
      }
      return { sessions, messages, toolCalls, streaming, errors, activeSessionId };
    });
  },

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
  appendThinking: (sessionId, token) =>
    set((st) => ({
      thinking: {
        ...st.thinking,
        [sessionId]: (st.thinking[sessionId] ?? "") + token,
      },
    })),
  clearThinking: (sessionId) =>
    set((st) => ({
      thinking: { ...st.thinking, [sessionId]: "" },
    })),
  setStreaming: (sessionId, streaming) =>
    set((st) => ({ streaming: { ...st.streaming, [sessionId]: streaming } })),
  setError: (sessionId, error) =>
    set((st) => ({ errors: { ...st.errors, [sessionId]: error } })),

  setModelLoading: (sessionId, loading) =>
    set((st) => ({ modelLoading: { ...st.modelLoading, [sessionId]: loading } })),

  setModelLoadingProgress: (sessionId, progress) =>
    set((st) => ({ modelLoadingProgress: { ...st.modelLoadingProgress, [sessionId]: progress } })),

  markModelReady: (sessionId) =>
    set((st) => ({ 
      modelLoading: { ...st.modelLoading, [sessionId]: false },
      modelLoadingProgress: { ...st.modelLoadingProgress, [sessionId]: 100 }
    })),

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
    // Désactiver le loading de modèle dès qu'on envoie un message (le backend gérera le vrai loading)
    set((s) => ({
      errors: { ...s.errors, [sessionId]: null },
      streaming: { ...s.streaming, [sessionId]: true },
      toolCalls: { ...s.toolCalls, [sessionId]: [] },
      thinking: { ...s.thinking, [sessionId]: "" },
      modelLoading: { ...s.modelLoading, [sessionId]: false },
      messages: {
        ...s.messages,
        [sessionId]: [
          ...(s.messages[sessionId] ?? []),
          { role: "user" as const, content: message },
        ],
      },
    }));
    try {
      const model = get().selectedModels[sessionId];
      const reasoning = get().reasoningLevels[sessionId] ?? "auto";
      await ipc.sessionSend({ sessionId, message, model, reasoning });
    } catch (e) {
      set((s) => ({
        streaming: { ...s.streaming, [sessionId]: false },
        errors: { ...s.errors, [sessionId]: String(e) },
        thinking: { ...s.thinking, [sessionId]: "" },
      }));
    }
  },

  cancel: async (sessionId) => {
    try {
      await ipc.sessionCancel({ sessionId });
    } catch {
      /* ignore */
    }
    set((s) => ({
      streaming: { ...s.streaming, [sessionId]: false },
      thinking: { ...s.thinking, [sessionId]: "" },
    }));
  },

  forkSession: async (sessionId) => {
    try {
      const forked = await ipc.sessionFork({ sessionId });
      if (!forked) return;
      // Restaure les messages de la fork (la DB les a déjà persistés).
      set((st) => ({
        sessions: [...st.sessions, forked],
        activeSessionId: forked.id,
        messages: { ...st.messages, [forked.id]: [] },
        toolCalls: { ...st.toolCalls, [forked.id]: [] },
        streaming: { ...st.streaming, [forked.id]: false },
        errors: { ...st.errors, [forked.id]: null },
      }));
      await get().restoreMessages(forked.id);
    } catch (e) {
      console.error("sessionFork error", e);
    }
  },
}));