# Architecture — Cyonima-ia-code-agent

## Vue d'ensemble

```
┌──────────────────────────────────────────────────────────┐
│  Frontend React (UI, thème violet unique)                  │
│  ├─ Sessions        (onglets multi-agents parallèles)      │
│  │   ├─ Bloc 1 (75%) : chat + raisonnement + réponses      │
│  │   │    └─ chatbox : modèle · raisonnement · contexte    │
│  │   └─ Bloc 2 (25%) : fichiers git du workspace           │
│  ├─ Catalogue        (installés + disponibles, tri RAM)    │
│  ├─ Ollama           (modèles installés + pull)            │
│  ├─ Config           (endpoint Ollama, permissions)        │
│  └─ Status bar       (version, session, modèle)            │
└───────────────────────┬────────────────────────────────────┘
                        │ Tauri IPC (commands + events)
┌───────────────────────┴────────────────────────────────────┐
│  Core Rust (src-tauri/src/)                                │
│  ├─ sessions/       gestionnaire multi-session parallèle   │
│  ├─ providers/      trait Provider (seul `ollama` actif)    │
│  │   └─ ollama      (HTTP vers Ollama local)               │
│  ├─ tools/          filesystem, bash, glob, grep            │
│  ├─ permissions/    gateway approbation utilisateur         │
│  ├─ config/         globale + par projet (TOML)             │
│  └─ ipc/            handlers Tauri (commands + events)      │
└────────────────────────────────────────────────────────────┘
```

> Le trait `Provider` reste générique pour permettre d'autres backends à
> l'avenir, mais **seul le provider Ollama est actif** dans cette version.

## Principes de design

1. **Provider abstrait** : le trait `Provider` découple l'UI/les sessions du backend. Seul `ollama` est implémenté et actif aujourd'hui.
2. **Multi-session native** : chaque session est un `tokio::task` indépendant avec son propre état, modèle, outils activés et permissions. Le streaming se fait via events Tauri (un event channel par session).
3. **Permissions explicites** : chaque appel d'outil passe par un gateway. Configurable global + per-project. Defauts prudents pour `bash`.
4. **Modèles gérés par Ollama** : aucun LLM n'est committé ni téléchargé par l'app. On délègue à Ollama (`ollama pull`) ; l'app liste et utilise ce qui est installé localement.
5. **Privacy first** : aucune télémétrie, aucune donnée sortante. Toute l'inférence passe par Ollama en local (`localhost:11434`).
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
    Thinking(String),   // reasoning des modèles « thinking » (DeepSeek R1, Qwen3, Gemma…)
    ToolCall(ToolCall),
    ToolResult(ToolResult),
    Done(Usage),
    Error(String),
}
```

Le `ChatRequest` transporte aussi un champ `reasoning: Option<String>`
(`"auto"`/`"off"`/`"low"`/`"medium"`/`"high"`) réglable depuis la chatbox.

### Détection de capacités Ollama

Le `OllamaProvider` interroge `POST /api/show` avant chaque conversation pour
lire les `capabilities` du modèle. Il n'envoie `tools` que si le modèle les
supporte (évite le HTTP 400 « does not support tools » de DeepSeek-R1). Un parseur
de secours extrait aussi le raisonnement inline `<think>…</think>` du champ
`content`.

### Intensité de raisonnement

Pour les modèles « thinking », le champ `think` d'Ollama est dérivé du
`reasoning` demandé : `auto` → `true`, `off` → `false`, `low`/`medium`/`high` →
niveau correspondant. Le champ n'est envoyé que si le modèle déclare la capacité
`thinking` (sinon Ollama renverrait une erreur). La taille de contexte du modèle
est lue via `POST /api/show` (`ollama_model_info`) pour alimenter l'indicateur
d'usage de contexte de la chatbox.

## Sessions

- **Session** = agent isolé (id, modèle, provider, contexte, tools, perms)
- **SessionManager** : pool, création/fork/cancel, persistance SQLite (`~/.cyonima/sessions.db`)
- Possibilité de **fork** une session (copie du contexte pour dévier une conversation)
- Une même fenêtre = N onglets = N sessions indépendantes (entre elles, même projet)
- **Création simplifiée** : le formulaire « Nouvelle session » ne demande que le
  répertoire de travail. Le provider est **Ollama** par défaut et le modèle se
  choisit ensuite dans un **menu déroulant du chat** (parmi les modèles installés
  dans Ollama). Le modèle courant est modifiable en cours de session : il est
  transmis à chaque `session_send` et stocké dans `SessionInner.current_model`.
- Le message `system` AGENTS.md est toujours injecté dans le contexte du LLM mais
  **masqué de l'affichage** (remplacé par un court message de bienvenue).
- Le modèle courant et l'intensité de raisonnement (`current_model`,
  `current_reasoning` dans `SessionInner`) sont modifiables en cours de session
  et transmis à chaque `session_send`.

### Interface de session (2 colonnes)

- **Bloc 1 (75%)** — conversation : réponses, bloc « Raisonnement du modèle »
  repliable, tool calls, puis la **chatbox**. La chatbox porte une barre de
  contrôles : sélecteur de modèle, menu d'intensité de raisonnement, et un
  **indicateur d'usage de contexte** (`tokensIn + tokensOut` du dernier tour vs
  taille de contexte du modèle). Boutons Play/Stop pour envoyer/arrêter.
- **Bloc 2 (25%)** — **fichiers git du workspace** : liste des fichiers ajoutés /
  modifiés / supprimés / renommés via `workspace_git_status` (`git status
  --porcelain`). Rafraîchi à la fin d'une génération et par sondage pendant
  qu'un agent travaille. On suppose que les workspaces sont des dépôts git.

## Modèles (via Ollama)

- Les modèles sont **entièrement gérés par Ollama**. Cyonima ne télécharge ni ne stocke de poids.
- **Lister** : `GET /api/tags` → `ollama_list_models` alimente le menu déroulant du chat.
- **Installer** : `POST /api/pull` en streaming → `ollama_pull_model`, avec events de progression (`ollama:pull:progress` / `:done` / `:error`).
- **Capacités** : `POST /api/show` détecte le support de `tools` et `thinking` par modèle (cf « Détection de capacités Ollama »).
- Un catalogue de tags suggérés vit dans `docs/models-catalog.toml` (informatif ; c'est Ollama qui fait foi sur ce qui est réellement installé).

## Permissions

| Outil | Permission par défaut |
|---|---|
| read_file, glob, grep | auto-approve |
| write_file, edit_file | demande |
| bash | demande + preview |

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
| Modèles | gérés par Ollama (hors de Cyonima) |

## IPC Tauri (V1)

### Commands (frontend → backend)
- `session_create { workspace, model_id, provider_id }` — `model_id` peut être vide (choisi ensuite dans le chat), `provider_id` = `ollama`
- `session_send { session_id, message, model?, reasoning? }` — `model` et `reasoning` = sélections de la chatbox
- `session_cancel { session_id }`
- `session_fork { session_id }`
- `session_history { session_id }` / `session_delete { session_id }` / `session_list {}`
- `ollama_list_models {}` (`GET /api/tags`) — alimente le menu déroulant du chat
- `ollama_pull_model { model }` (`POST /api/pull` streaming)
- `ollama_model_info { model }` (`POST /api/show`) → `{ contextLength }` pour l'indicateur de contexte
- `workspace_git_status { workspace }` → `{ isRepo, changes[] }` (fichiers modifiés, `git status --porcelain`)
- `hardware_get {}` → snapshot RAM/CPU/OS/arch/VRAM
- `hardware_can_run_model { ram_min_gb }` → bool (adéquation modèle / machine)
- `config_get {}` / `config_get_workspace { workspace }` / `config_set_*`
- `permission_respond { request_id, decision }`

> Note : tous les payloads d'events de session sont sérialisés en **camelCase**
> (`#[serde(rename_all = "camelCase")]`) pour matcher le frontend TypeScript
> (`sessionId`, `callId`, `isError`, `tokensIn`…).

### Events (backend → frontend)
- `session:token { sessionId, token }`
- `session:thinking { sessionId, token }` — reasoning streamé (affiché dans un bloc repliable)
- `session:tool_call { sessionId, callId, tool, arguments }`
- `session:tool_result { sessionId, callId, tool, output, isError }`
- `session:model_loading { sessionId, loading, progress }`
- `session:done { sessionId, usage }`
- `session:error { sessionId, error }`
- `ollama:pull:progress { model, status, total, completed }` (+ `:done` / `:error`)
- `permission:request { requestId, sessionId, tool, arguments, preview? }`