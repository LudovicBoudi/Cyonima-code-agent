# Architecture — Cyonima-ia-code-agent

## Vue d'ensemble

```
┌──────────────────────────────────────────────────────────┐
│  Frontend React (UI, thème violet unique)                  │
│  ├─ Sessions        (onglets multi-agents parallèles)      │
│  │   ├─ Bloc 1 (50%) : chat + réponses + outils            │
│  │   │    └─ chatbox : modèle · raisonnement · contexte    │
│  │   ├─ Bloc 2 (25%) : ThinkingPanel (raisonnement)        │
│  │   └─ Bloc 3 (25%) : fichiers git du workspace           │
│  ├─ Catalogue        (tags Ollama, tri RAM)                │
│  ├─ Ollama           (modèles installés + pull)            │
│  ├─ Importer         (import de GGUF custom)               │
│  ├─ Clés API         (keyring OS — providers distants)     │
│  ├─ Configuration    (endpoint Ollama, permissions)        │
│  ├─ Recherche        (recherche sémantique — désactivée)   │
│  └─ Status bar       (version, session, modèle)            │
└───────────────────────┬────────────────────────────────────┘
                        │ Tauri IPC (commands + events)
┌───────────────────────┴────────────────────────────────────┐
│  Core Rust (src-tauri/src/)                                │
│  ├─ sessions/       gestionnaire multi-session parallèle   │
│  ├─ providers/      trait Provider (seul `ollama` actif)    │
│  │   ├─ ollama      (HTTP vers Ollama local)               │
│  │   └─ (inactifs)  llama_cpp, openai, anthropic, gemini,  │
│  │                  openai_compat — présents mais désactivés│
│  ├─ tools/          filesystem, bash, glob, grep, search   │
│  ├─ permissions/    gateway approbation utilisateur         │
│  ├─ config/         globale + par projet (TOML)             │
│  ├─ secrets/        clés API via keyring OS                 │
│  ├─ models/         registry + catalogue Ollama             │
│  ├─ hardware/       garde-fou RAM/VRAM                      │
│  ├─ indexing/       recherche sémantique (embedder désactivé)│
│  └─ ipc/            handlers Tauri (commands + events)      │
└────────────────────────────────────────────────────────────┘
```

> Le trait `Provider` reste générique pour permettre d'autres backends à
> l'avenir, mais **seul le provider Ollama est actif** dans cette version.
> Les implémentations `llama_cpp`, `openai`, `anthropic`, `gemini` et
> `openai_compat` sont présentes dans le code mais **désactivées** (non
> sélectionnables comme provider de session).

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

### Interface de session (3 colonnes)

- **Bloc 1 (50%)** — conversation : réponses, tool calls (avec résultat
  repliable), puis la **chatbox**. La chatbox porte une barre de
  contrôles : sélecteur de modèle, menu d'intensité de raisonnement, et un
  **indicateur d'usage de contexte** (`tokensIn + tokensOut` du dernier tour vs
  taille de contexte du modèle). Boutons Play/Stop pour envoyer/arrêter.
- **Bloc 2 (25%)** — **ThinkingPanel** : flux de tokens « thinking » du modèle
  en temps réel, avec auto-scroll et indicateur de streaming. Le panneau se
  vide et affiche « Le raisonnement s'affichera ici... » quand le modèle
  ne produit pas de thinking.
- **Bloc 3 (25%)** — **fichiers git du workspace** : liste des fichiers ajoutés /
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

| Outil | Permission par défaut | Label dans le system prompt |
|---|---|---|
| read_file, glob, grep | auto-approve | `Auto-approuvé` |
| write_file, edit_file | auto-approve | `Auto-approuvé` |
| bash | demande + preview | `Nécessite approbation` |

Mécanisme : chaque tool call est enveloppé. Le gateway check `config.permissions.<tool>` puis utilise si besoin le `Command` Tauri `permission:request` qui affiche un dialogue UI.

## Secrets (clés API)

Les clés des providers distants (`openai`, `anthropic`, `gemini`,
`openai_compat`) sont stockées dans le **keyring OS** (DPAPI / Keychain /
Secret Service) via le module `secrets/`. Elles ne sont jamais écrites en clair
sur disque, jamais loggées, jamais sérialisées dans la config. Ce module est
présent pour les backends distants, **inactifs dans cette version** (Ollama
n'utilise pas de clé).

## Indexation sémantique (désactivée)

Le module `indexing/` implémente un pipeline complet :
`fichier → chunks → embeddings (candle BERT all-MiniLM-L6-v2) → SQLite →
recherche`, exposé via les IPC `index_build` / `index_search` / `index_count`,
la page **Recherche** et l'outil agent `semantic_search`.

> **Statut** : l'embedder est **désactivé** dans cette version
> (`Embedder::load()` renvoie une erreur) pour éviter les téléchargements
> automatiques qui déclenchent Windows Defender. Le code est conservé pour une
> réactivation ultérieure.

## Configuration

- **Global** : `~/.cyonima/config.toml`
- **Par projet** : `<workspace>/.cyonima/config.toml` (override)
- **AGENTS.md** (à la racine du workspace) : instructions personnalisées injectées dans le system prompt

## Stockage runtime

| Type | Emplacement |
|---|---|
| Config globale | `~/.cyonima/config.toml` |
| Sessions DB | `~/.cyonima/sessions.db` (SQLite) |
| Registry modèles (GGUF importés) | `~/.cyonima/models/registry.json` |
| Index sémantique (désactivé) | `<data_local>/cyonima/index/search.db` (SQLite) |
| Cache embedder (désactivé) | `<data_local>/cyonima/models/embedder/` |
| Modèles | gérés par Ollama (hors de Cyonima) |

## IPC Tauri (V1)

### Commands (frontend → backend)

**Sessions**
- `session_create { workspace, model_id, provider_id }` — `model_id` peut être vide (choisi ensuite dans le chat), `provider_id` = `ollama`
- `session_send { session_id, message, model?, reasoning? }` — `model` et `reasoning` = sélections de la chatbox
- `session_cancel { session_id }`
- `session_fork { session_id }`
- `session_history { session_id }` / `session_delete { session_id }` / `session_list {}`

**Ollama**
- `ollama_list_models {}` (`GET /api/tags`) — alimente le menu déroulant du chat
- `ollama_pull_model { model }` (`POST /api/pull` streaming)
- `ollama_delete_model { model }` (`DELETE /api/delete`) — supprime un modèle de Ollama
- `ollama_model_info { model }` (`POST /api/show`) → `{ contextLength }` pour l'indicateur de contexte

**Modèles & catalogue**
- `model_catalog_list {}` → catalogue embarqué (tags Ollama) avec statut `installed`
- `model_list_installed {}` → modèles du registry encore présents sur disque
- `model_import_custom { path }` → import d'un GGUF custom dans le registry
- `model_clear_cache {}` → vide registry + dossier `~/.cyonima/models`
- `model_download` / `model_download_cancel` — **obsolètes** (stubs qui renvoient une erreur ; le téléchargement passe par Ollama)

**Providers distants (keyring — inactifs dans cette version)**
- `provider_set_api_key { provider, api_key }` / `provider_get_api_key` / `provider_has_api_key` / `provider_delete_api_key`
- `provider_list_configured {}`

**Hardware**
- `hardware_get {}` → snapshot RAM/CPU/OS/arch/VRAM
- `hardware_can_run_model { ram_min_gb }` → bool (adéquation modèle / machine)

**Config**
- `config_get {}` / `config_get_workspace { workspace }`
- `config_set_default_provider` / `config_set_default_model` / `config_set_ollama_endpoint`
- `config_set_permission` / `config_remove_permission`

**Permissions**
- `permission_respond { request_id, decision }`

**Git & fichiers**
- `workspace_git_status { workspace }` → `{ isRepo, changes[] }` (fichiers modifiés, `git status --porcelain`)
- `file_revert { workspace, path, content }` → réécrit un fichier (annulation d'un edit/write depuis le diff viewer)

**Indexation sémantique (embedder désactivé)**
- `index_build { workspace }` / `index_search { workspace, query, limit? }` / `index_count {}`

> Note : tous les payloads d'events de session sont sérialisés en **camelCase**
> (`#[serde(rename_all = "camelCase")]`) pour matcher le frontend TypeScript
> (`sessionId`, `callId`, `isError`, `tokensIn`…). Ceci inclut
> `PermissionRequest.request_id` (fixé en v1.1.0 — manquait `rename_all`).

### Events (backend → frontend)
- `session:token { sessionId, token }`
- `session:thinking { sessionId, token }` — reasoning streamé (affiché dans le ThinkingPanel)
- `session:tool_call { sessionId, callId, tool, arguments }`
- `session:tool_result { sessionId, callId, tool, output, isError }`
- `session:model_loading { sessionId, loading, progress }`
- `session:done { sessionId, usage }` — **toujours émis**, même en cas d'erreur ou d'annulation
- `session:error { sessionId, error }`
- `ollama:pull:progress { model, status, total, completed }` (+ `:done` / `:error`)
- `permission:request { requestId, sessionId, tool, arguments, preview? }`

### Workspace Snapshot

À la création ou restauration d'une session, un **snapshot du workspace** est
injecté comme premier message `system` dans le contexte LLM. Le snapshot contient
la structure d'arborescence (depth 2, ignore `.git/target/node_modules/`) et le
contenu des fichiers de config clés (`Cargo.toml`, `package.json`, `README.md`,
`AGENTS.md`, etc.) — ceci donne au LLM une compréhension immédiate du projet
sans avoir à appeler d'outils.

### Permissions Logging

Chaque demande de permission est loguée (`tracing::info!`) avec son
`request_id`, et chaque réponse est loguée avec la décision. En cas de
`request_id` inconnu, un warning est émis. Ceci aide au diagnostic quand le
dialogue de permission n'apparaît pas.

### Modèle courant modifiable en session

Le modèle courant est stocké dans `SessionInner.current_model` et transmis à
chaque `session_send`. Il est modifiable via un menu déroulant dans la chatbox.
Un IPC `session_set_model` permet de le changer sans renvoyer de message.