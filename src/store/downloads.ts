import { create } from "zustand";
import { ipc } from "../lib/ipc";

export interface DownloadState {
  modelId: string;
  downloaded: number;
  total: number;
  bytesPerSecond: number;
  error?: string;
  done?: boolean;
}

interface DownloadsStore {
  downloads: Record<string, DownloadState>;

  setProgress: (p: DownloadState) => void;
  markDone: (modelId: string) => void;
  markError: (modelId: string, error: string) => void;
  remove: (modelId: string) => void;

  /// Lance un téléchargement. La progression vient via l'event
  /// `model:download:progress` écouté par App.tsx.
  start: (modelId: string) => Promise<void>;
  /// Annule un téléchargement en cours (préserve le `.part` pour reprise future).
  cancel: (modelId: string) => Promise<void>;
}

export const useDownloadsStore = create<DownloadsStore>((set, get) => ({
  downloads: {},

  setProgress: (p) =>
    set((st) => ({
      downloads: {
        ...st.downloads,
        [p.modelId]: { ...st.downloads[p.modelId], ...p, done: false, error: undefined },
      },
    })),
  markDone: (modelId) =>
    set((st) => ({
      downloads: {
        ...st.downloads,
        [modelId]: {
          ...(st.downloads[modelId] ?? {
            modelId,
            downloaded: 0,
            total: 0,
            bytesPerSecond: 0,
          }),
          done: true,
          error: undefined,
        },
      },
    })),
  markError: (modelId, error) =>
    set((st) => ({
      downloads: {
        ...st.downloads,
        [modelId]: {
          ...(st.downloads[modelId] ?? {
            modelId,
            downloaded: 0,
            total: 0,
            bytesPerSecond: 0,
          }),
          error,
          done: false,
        },
      },
    })),
  remove: (modelId) =>
    set((st) => {
      const next = { ...st.downloads };
      delete next[modelId];
      return { downloads: next };
    }),

  start: async (modelId) => {
    // optimistic : affiche un état "prend..." avant le premier progress event
    set((st) => ({
      downloads: {
        ...st.downloads,
        [modelId]: { modelId, downloaded: 0, total: 0, bytesPerSecond: 0 },
      },
    }));
    try {
      await ipc.modelDownload({ modelId });
    } catch (e) {
      get().markError(modelId, String(e));
    }
  },

  cancel: async (modelId) => {
    try {
      await ipc.modelDownloadCancel({ modelId });
    } catch {
      /* ignore */
    }
    // On garde l'entrée dans le store mais marquée comme stoppée via
    // l'event error "annulé" qui va arriver. On ne retire pas ici pour
    // que l'UI affiche "Annulé" avec un titre clair.
  },
}));