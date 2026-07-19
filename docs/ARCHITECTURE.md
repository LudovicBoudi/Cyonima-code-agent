# Architecture — Cyonima-ia-code-agent

## Vue d'ensemble

```
┌──────────────────────────────────────────────────────────┐
│  Frontend React (UI)                                       │
│  ├─ Projects        (ouverture / workspace)               │
│  ├─ Sessions        (onglets multi-agents parallèles)      │
│  ├─ Models           (installés, catalogue, import custom) │
│  ├─ Settings         (providers, API keys, storage, thème)  │
│  └─ Status bar       (session, modèle, provider, tokens)   │
└───────────────────────┬────────────────────────────────────┘
                        │ Tauri IPC (commands + events)
┌───────────────────────┴────────────────────────────────────┐
│  Core Rust (src-tauri/src/)                                │
│  ├─ sessions/       gestionnaire multi-session parallèle   │
│  ├─ providers/      trait Provider + impls                 │
│  │   ├─ llama_cpp   (inférence locale via bindings C)       │
│  │   ├─ ollama      (HTTP vers Ollama externe)              │
│  │   ├─ openai      (API distante)                          │
│  │   ├─ anthropic                                      │
│  │   ├─ gemini                                          │
│  │   └─ openai_compat  (LM Studio, vLLM, entreprise)        │
│  ├─ models/         registry + downloader + import custom   │
│  ├─ tools/          filesystem, bash, glob, grep            │
│  ├─ permissions/    gateway approbation utilisateur         │
│  ├─ config/         globale + par projet (TOML)             │
│  ├─ indexing/       embedder + SQLite + recherche           │
│  └─ ipc/            handlers Tauri (commands + events)      │
└────────────────────────────────────────────────────────────┘
```

## Principes de design

1. **Provider abstrait** : tout backend (local ou distant) implémente le trait `Provider`. L'UI et les sessions sont agnostiques du modèle.
2. **Multi-session native** : chaque session est un `tokio::task` indépendant avec son propre état, modèle, outils activés et permissions. Le streaming se fait via events Tauri (un event channel par session).
3. **Permissions explicites** : chaque appel d'outil passe par un gateway. Configurable global + per-project. Defauts prudents pour `bash`.
4. **Pas de gros fichiers dans le repo** : seules les ressources < 50 Mo (embedder) sont committées. Tout LLM est téléchargé à l'exécution ou importé.
5. **Privacy first** : aucune télémétrie, aucune donnée sortante non sollicitée. Les clés API vivent dans le keyring OS.
6. **Reproductibilité** : versions épinglées (Cargo.lock + package-lock), CI multi-OS.

## Trait `Provider`

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    async fn stream(&self, req: ChatRequest) -> BoxStream<'static, ChatEvent>;
    fn capabilities(&self) -> Capabilities; // tools, vision, context_window
    fn id(&self) -> &str;
}

pub enum ChatEvent {
    Token(String),
    ToolCall(ToolCall),
    ToolResult(ToolResult),
    Done(Usage),
    Error(String),
}
```

## Sessions

- **Session** = agent isolé (id, modèle, provider, contexte, tools, perms)
- **SessionManager** : pool, création/fork/cancel, persistance SQLite (`~/.cyonima/sessions.db`)
- Possibilité de **fork** une session (copie du contexte pour dévier une conversation)
- Une même fenêtre = N onglets = N sessions indépendantes (entre elles, même projet)

## Modèles

### Sources (cahier des charges)
1. **< 50 Mo** : embarqués (`src-tauri/resources/`). Actuellement : embedder `all-MiniLM-L6-v2` Q8.
2. **> 50 Mo open source** : catalogue téléchargeable (`docs/models-catalog.toml` → V1: HuggingFace Hub + Ollama Library)
3. **Entreprise / custom** : UI d'import → metadata + chemin enregistrés dans le registry local.

### Téléchargeur
- `reqwest` avec HTTP range pour reprise sur interruption
- SHA256 post-download (checksum issu du catalogue)
- Progression via events Tauri (`model:download:progress`)
- Pause / cancel via `CancellationToken`
- Vérification d'espace disque avant lancement

### Registry local
- `~/.cyonima/models/registry.json` : modèles installés, sources, licences, tailles, RAM min recommandée
- Stockage path configurable (global ou par projet)

## Permissions

| Outil | Permission par défaut |
|---|---|
| read_file, glob, grep | auto-approve |
| write_file, edit_file | demande |
| bash | demande + preview |
| import custom model | auto-approve |

Mécanisme : chaque tool call est enveloppé. Le gateway check `config.permissions.<tool>` puis utilise si besoin le `Command` Tauri `permission:request` qui affiche un dialogue UI.

## Configuration

- **Global** : `~/.cyonima/config.toml`
- **Par projet** : `<workspace>/.cyonima/config.toml` (override)
- **AGENTS.md** (à la racine du workspace) : instructions personnalisées injectées dans le system prompt

## Stockage runtime

| Type | Emplacement |
|---|---|
| Config globale | `~/.cyonima/config.toml` |
| Sessions DB | `~/.cyonima/sessions.db` (SQLite) |
| Embeddings index | `~/.cyonima/index/<project-hash>/` |
| Modèles | configurable (défaut `~/.cyonima/models/`) |
| API keys | keyring OS |

## IPC Tauri (V1)

### Commands (frontend → backend)
- `hardware_get {}` → snapshot RAM/CPU/OS/arch/VRAM (si détectable)
- `hardware_can_run_model { ram_min_gb }` → bool pour griser/autoriser un download
- `session_create { workspace, model, provider }`
- `session_send { session_id, message }`
- `session_cancel { session_id }`
- `session_fork { session_id }`
- `model_list_installed {}`
- `model_catalog_list {}`
- `model_download { model_id, ram_min_gb? }` — garde-fou hardware en backend, `Err` si RAM < requis+1 Go
- `model_download_cancel { model_id }`
- `model_import_custom { path }`
- `provider_set_api_key { provider, key }`  (via keyring)
- `config_get`, `config_set`
- `permission_respond { request_id, decision }`
- `hardware_get {}` → RAM/CPU/OS/arch/VRAM
- `hardware_can_run_model { ram_min_gb }` → bool pour griser le bouton Télécharger

### Events (backend → frontend)
- `session:token { session_id, token }`
- `session:tool_call { session_id, call }`
- `session:done { session_id, usage }`
- `session:error { session_id, error }`
- `model:download:progress { model_id, bytes, total }`
- `permission:request { request_id, tool, args }`