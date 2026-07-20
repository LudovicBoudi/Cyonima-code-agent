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

export interface DownloadProgressEvent {
  modelId: string;
  downloaded: number;
  total: number;
  bytesPerSecond: number;
}

export interface DownloadDoneEvent {
  modelId: string;
  path: string;
  sha256: string;
  sizeBytes: number;
}

export interface DownloadErrorEvent {
  modelId: string;
  error: string;
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

export interface OllamaModelInfo {
  name: string;
  size: number;
  digest: string;
  modifiedAt: string;
}

export interface OllamaPullProgress {
  status: string;
  completed?: boolean;
  total?: number;
  digest?: string;
}

// ===== Commands =====

export const ipc = {
  sessionCreate: (p: { workspace: string; modelId: string; providerId: string }) =>
    invoke<SessionInfo>("session_create", p),
  sessionSend: (p: { sessionId: string; message: string }) =>
    invoke<void>("session_send", p),
  sessionCancel: (p: { sessionId: string }) => invoke<void>("session_cancel", p),
  sessionFork: (p: { sessionId: string }) => invoke<SessionInfo | null>("session_fork", p),
  sessionHistory: (p: { sessionId: string }) =>
    invoke<ChatMessage[]>("session_history", p),
  sessionDelete: (p: { sessionId: string }) => invoke<void>("session_delete", p),
  sessionList: () => invoke<SessionInfo[]>("session_list"),

  modelListInstalled: () => invoke<ModelInfo[]>("model_list_installed"),
  modelCatalogList: () => invoke<ModelInfo[]>("model_catalog_list"),
  modelDownload: (p: { modelId: string }) => invoke<void>("model_download", p),
  modelDownloadCancel: (p: { modelId: string }) =>
    invoke<void>("model_download_cancel", p),
  modelImportCustom: (p: { path: string }) =>
    invoke<ModelInfo>("model_import_custom", p),

  permissionRespond: (p: { requestId: string; decision: "allow" | "deny" }) =>
    invoke<void>("permission_respond", p),

  providerSetApiKey: (p: { provider: string; apiKey: string }) =>
    invoke<void>("provider_set_api_key", p),
  providerGetApiKey: (p: { provider: string }) =>
    invoke<string | null>("provider_get_api_key", p),
  providerHasApiKey: (p: { provider: string }) =>
    invoke<boolean>("provider_has_api_key", p),
  providerDeleteApiKey: (p: { provider: string }) =>
    invoke<void>("provider_delete_api_key", p),
  providerListConfigured: () =>
    invoke<string[]>("provider_list_configured"),

  ollamaListModels: () =>
    invoke<OllamaModelInfo[]>("ollama_list_models"),
  ollamaPullModel: (p: { model: string }) =>
    invoke<void>("ollama_pull_model", p),

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

export function onDownloadProgress(cb: (e: DownloadProgressEvent) => void): Promise<UnlistenFn> {
  return listen("model:download:progress", (ev) => cb(ev.payload as never));
}

export function onDownloadDone(cb: (e: DownloadDoneEvent) => void): Promise<UnlistenFn> {
  return listen("model:download:done", (ev) => cb(ev.payload as never));
}

export function onDownloadError(cb: (e: DownloadErrorEvent) => void): Promise<UnlistenFn> {
  return listen("model:download:error", (ev) => cb(ev.payload as never));
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

export function onOllamaPullProgress(cb: (e: OllamaPullProgress & { model: string }) => void): Promise<UnlistenFn> {
  return listen("ollama:pull:progress", (ev) => cb(ev.payload as never));
}

export function onOllamaPullDone(cb: (e: { model: string }) => void): Promise<UnlistenFn> {
  return listen("ollama:pull:done", (ev) => cb(ev.payload as never));
}

export function onOllamaPullError(cb: (e: { model: string; error: string }) => void): Promise<UnlistenFn> {
  return listen("ollama:pull:error", (ev) => cb(ev.payload as never));
}