import { create } from "zustand";
import type { PermissionRequestEvent } from "../lib/ipc";

export interface PendingPermission {
  request: PermissionRequestEvent;
  decided: boolean;
}

interface PermissionsState {
  queue: PendingPermission[];
  enqueue: (req: PermissionRequestEvent) => void;
  markDecided: (requestId: string) => void;
  remove: (requestId: string) => void;
}

export const usePermissionsStore = create<PermissionsState>((set) => ({
  queue: [],
  enqueue: (req) =>
    set((s) => ({ queue: [...s.queue, { request: req, decided: false }] })),
  markDecided: (requestId) =>
    set((s) => ({
      queue: s.queue.map((p) =>
        p.request.requestId === requestId ? { ...p, decided: true } : p,
      ),
    })),
  remove: (requestId) =>
    set((s) => ({
      queue: s.queue.filter((p) => p.request.requestId !== requestId),
    })),
}));
