import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ===== Types IPC V1 (cf docs/ARCHITECTURE.md) =====

export interface SessionId {
  id: string;
}

export interface ChatMessage {
  role: "system" | "user" | "assistant" | "tool";
  content: string;
}

export interface SessionInfo {
  id: string;
  workspace: string;
  modelId: string;
  providerId: string;
  createdAt: number;
}

export interface ModelInfo {
  id: string;
  name: string;
  sizeBytes: number;
  quantization: string;
  license: string;
  ramMinGb: number;
  installed: boolean;
  ollamaTag?: string;
  url?: string;
}

export interface DownloadProgress {
  modelId: string;
  bytes: number;
  total: number;
}

export interface HardwareInfo {
  totalRamBytes: number;
  totalRamGb: number;
  cpuCores: number;
  os: string;
  arch: string;
  vramBytes: number | null;
  vramGb: number;
}

// ===== Commands =====

export const ipc = {
  sessionCreate: (p: { workspace: string; modelId: string; providerId: string }) =>
    invoke<SessionInfo>("session_create", p),
  sessionSend: (p: { sessionId: string; message: string }) =>
    invoke<void>("session_send", p),
  sessionCancel: (p: { sessionId: string }) => invoke<void>("session_cancel", p),
  sessionFork: (p: { sessionId: string }) => invoke<SessionInfo>("session_fork", p),
  sessionList: () => invoke<SessionInfo[]>("session_list"),

  modelListInstalled: () => invoke<ModelInfo[]>("model_list_installed"),
  modelCatalogList: () => invoke<ModelInfo[]>("model_catalog_list"),
  modelDownload: (p: { modelId: string; ramMinGb?: number }) =>
    invoke<void>("model_download", p),
  modelDownloadCancel: (p: { modelId: string }) =>
    invoke<void>("model_download_cancel", p),
  modelImportCustom: (p: { path: string }) =>
    invoke<ModelInfo>("model_import_custom", p),

  permissionRespond: (p: { requestId: string; decision: "allow" | "deny" }) =>
    invoke<void>("permission_respond", p),

  hardwareGet: () => invoke<HardwareInfo>("hardware_get"),
  hardwareCanRunModel: (ramMinGb: number) =>
    invoke<boolean>("hardware_can_run_model", { ramMinGb }),
};

// ===== Events helpers =====

export function onSessionToken(cb: (e: { sessionId: string; token: string }) => void): Promise<UnlistenFn> {
  return listen("session:token", (ev) => cb(ev.payload as never));
}

export function onSessionDone(
  cb: (e: { sessionId: string; usage: { tokensIn: number; tokensOut: number } }) => void,
): Promise<UnlistenFn> {
  return listen("session:done", (ev) => cb(ev.payload as never));
}

export function onSessionError(
  cb: (e: { sessionId: string; error: string }) => void,
): Promise<UnlistenFn> {
  return listen("session:error", (ev) => cb(ev.payload as never));
}

export function onSessionToolCall(
  cb: (e: { sessionId: string; callId: string; tool: string; arguments: unknown }) => void,
): Promise<UnlistenFn> {
  return listen("session:tool_call", (ev) => cb(ev.payload as never));
}

export function onSessionToolResult(
  cb: (e: { sessionId: string; callId: string; tool: string; output: string; isError: boolean }) => void,
): Promise<UnlistenFn> {
  return listen("session:tool_result", (ev) => cb(ev.payload as never));
}

export function onDownloadProgress(cb: (e: DownloadProgress) => void): Promise<UnlistenFn> {
  return listen("model:download:progress", (ev) => cb(ev.payload as never));
}

export interface PermissionRequestEvent {
  requestId: string;
  sessionId: string;
  tool: string;
  arguments: unknown;
  preview?: string;
}

export function onPermissionRequest(cb: (e: PermissionRequestEvent) => void): Promise<UnlistenFn> {
  return listen("permission:request", (ev) => cb(ev.payload as never));
}