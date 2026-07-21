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
  modelType: string;
  installed: boolean;
  installedPath?: string;
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

export interface OllamaModelDetails {
  contextLength: number | null;
}

/// Niveaux d'intensité de raisonnement exposés dans l'UI.
export type ReasoningLevel = "auto" | "off" | "low" | "medium" | "high";

export interface GlobalConfig {
  storage: { modelsDir: string };
  permissions: { overrides: Record<string, string> };
  provider: {
    defaultProvider: string | null;
    defaultModel: string | null;
    ollamaEndpoint: string | null;
  };
}

export interface IndexStats {
  filesScanned: number;
  chunksCreated: number;
  chunksEmbedded: number;
  errors: string[];
}

export interface SearchResult {
  filePath: string;
  startLine: number;
  endLine: number;
  text: string;
  score: number;
}

export type GitFileStatus = "added" | "modified" | "deleted" | "renamed" | "untracked";

export interface GitFileChange {
  path: string;
  status: GitFileStatus;
}

export interface GitStatus {
  isRepo: boolean;
  changes: GitFileChange[];
}

// ===== Commands =====

export const ipc = {
  sessionCreate: (p: { workspace: string; modelId: string; providerId: string }) =>
    invoke<SessionInfo>("session_create", p),
  sessionSend: (p: { sessionId: string; message: string; model?: string; reasoning?: string }) =>
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
  modelClearCache: () => invoke<void>("model_clear_cache"),
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
  ollamaModelInfo: (p: { model: string }) =>
    invoke<OllamaModelDetails>("ollama_model_info", p),

  configGet: () => invoke<GlobalConfig>("config_get"),
  configGetWorkspace: (p: { workspace: string }) =>
    invoke<GlobalConfig>("config_get_workspace", p),
  configSetDefaultProvider: (p: { provider: string | null }) =>
    invoke<void>("config_set_default_provider", p),
  configSetDefaultModel: (p: { model: string | null }) =>
    invoke<void>("config_set_default_model", p),
  configSetOllamaEndpoint: (p: { endpoint: string | null }) =>
    invoke<void>("config_set_ollama_endpoint", p),
  configSetPermission: (p: { tool: string; policy: string }) =>
    invoke<void>("config_set_permission", p),
  configRemovePermission: (p: { tool: string }) =>
    invoke<void>("config_remove_permission", p),

  hardwareGet: () => invoke<HardwareInfo>("hardware_get"),
  hardwareCanRunModel: (ramMinGb: number) =>
    invoke<boolean>("hardware_can_run_model", { ramMinGb }),

  indexBuild: (p: { workspace: string }) =>
    invoke<IndexStats>("index_build", p),
  indexSearch: (p: { workspace: string; query: string; limit?: number }) =>
    invoke<SearchResult[]>("index_search", p),
  indexCount: () => invoke<number>("index_count"),

  workspaceGitStatus: (p: { workspace: string }) =>
    invoke<GitStatus>("workspace_git_status", p),
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

export function onSessionThinking(
  cb: (e: { sessionId: string; token: string }) => void,
): Promise<UnlistenFn> {
  return listen("session:thinking", (ev) => cb(ev.payload as never));
}

export function onSessionModelLoading(
  cb: (e: { sessionId: string; loading: boolean; progress: number }) => void,
): Promise<UnlistenFn> {
  return listen("session:model_loading", (ev) => cb(ev.payload as never));
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