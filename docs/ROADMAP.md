# Roadmap — Cyonima-ia-code-agent v1

Les jalons sont prévus pour être livrés dans l'ordre. Chacun a des critères de définition (DoD) clairs.

> **Recentrage Ollama (version courante)** — Le produit se limite désormais aux
> **capacités d'Ollama** : l'inférence, la gestion et le téléchargement des
> modèles passent tous par Ollama. Les fonctionnalités suivantes des jalons ci-dessous
> sont **désactivées / reportées** dans cette version : backend candle GGUF intégré
> (J1.5/J4.5), downloader GGUF direct (J4), import de GGUF custom (J5), providers
> d'API distantes OpenAI/Anthropic/Gemini/OpenAI-compat (J6) et recherche
> sémantique via embedder local (J8). L'historique ci-dessous est conservé à titre
> de référence de conception.

## J0 — Squelette & socle technique  ✅ (en cours)
- Repo Tauri 2 + React + TypeScript + Tailwind
- Structure de modules Rust (skeletons)
- Docs : README, ARCHITECTURE, ROADMAP, AGENTS
- LICENSE MIT, configs (rustfmt, clippy, editorconfig, gitignore)
- CI GitHub Actions multi-OS (lint + build)
- Catalogue de modèles (TOML) + placeholder embedder
- **DoD** : `npm run tauri build` s'exécute sur les 3 OS.

## J1 — Provider trait + 1 provider local  ✅
- Trait `Provider` + `ChatRequest` / `ChatEvent`  ✅
- Impl `OllamaProvider` HTTP streaming NDJSON (fonctionnel)  ✅
- Impl `LlamaCppProvider` : stub conforme au trait, **J1.5** dédié au câblage candle  ⏳
- `SessionManager` : pool in-memory, tokio task par envoi, events `session:token/done/error`, `CancellationToken`, fork  ✅
- IPC `session_create` / `session_send` / `session_cancel` / `session_fork` / `session_list`  ✅
- UI : formulaire "Nouvelle session", chat input + bouton Stop, stream display, listeners d'events  ✅
- **DoD J1** : chat token-by-token fonctionne via Ollama local (cf README).

## J1.5 — Détection VRAM GPU + catalogue réel  ✅
- Module `hardware/vram.rs` :
  - **Windows DXGI** `EnumAdapters1` + `DedicatedVideoMemory` (corrige le bug Win32_VideoController > 4 Go)
  - **Linux sysfs** `/sys/class/drm/card*/device/mem_info_vram_total` (amdgpu)
  - **macOS** `system_profiler SPDisplaysDataType`
- `HardwareInfo` étendu (`vram_bytes`, `vram_gb`)
- `can_run_model` **relaxe** le seuil RAM si le modèle tient entièrement en VRAM
- Vérifié sur i9-13900K + Arc A770 16 Go : VRAM correctement détectée (16 GiB), garde-fou OK
- **Parser catalogue** `docs/models-catalog.toml` embarqué via `include_str!` (compile-time, 0 IO)
- IPC `model_catalog_list` + UI Catalogue avec badges "OK / RAM insuff." + tags Ollama
- Gemma 4 (E2B/E4B/12B/26B-A4B/31B) ajoutés avec licences **Apache-2.0** + tags `gemma4:*` Ollama
- Tests Rust : 3 (catalogue non vide, IDs uniques, Gemma 4 Apache + tag Ollama, import_custom reject)
- **J1.5 suite (candle built-in)** : reporté au jalon qui permette un test réel contre un GGUF (J4 downloader). On ne livre pas candle au doigt mouillé.

## J2 — Multi-session  ✅
- `sessions/persistence.rs` : SQLite `~/.cyonima/sessions.db`
  - Tables `sessions` + `messages` avec `ON DELETE CASCADE`
  - Journal mode `WAL` pour permettre multi-session lecture/écriture concurrente
  - FOREIGN_KEYS activées (CASCADE delete messages sur delete session)
  - Atomic flush, pool Sqlx partagé via `AppState`
  - 4 tests Rust : upsert/list, append/load messages, delete cascade, set/get title
- `SessionManager` extension :
  - `with_persistence(Persistence)` + `restore_all()` au démarrage
  - `create/fork/send/delete` deviennent async et persistent (session + messages)
  - Message `AGENTS.md` system n'est **pas** persisté : rechargé à chaque `restore_all`
  - Chaque message `user`, `assistant` (post-stream), `tool` (résultat + refus) est flushé
  - En cas d'erreur DB on log + continue (la session reste fonctionnelle en mémoire)
- 3 nouvelles IPC : `session_history(session_id)` → `Vec<ChatMessage>`, `session_delete(session_id)`, `session_fork` async
- UI SessionManager (Zustand) :
  - `loadAll()` appelé au démarrage App → `sessionList()` + restaure la plus récente active
  - `restoreMessages(sessionId)` recharge l'historique quand l'utilisateur switch d'onglet vers une session non encore chargée
  - `deleteSession(sessionId)` supprime localement et switch vers la suivante
  - `forkSession(sessionId)` crée une nouvelle session avec le même contexte + restore ses messages
- UI Sidebar :
  - Compteur "Sessions (N)" dans la nav
  - Boutons hover par session : ✕ (supprimer) + Fork
  - « Sessions récentes » section en dessous des liens Sessions / Catalogue
- **DoD J2** : 2 sessions parallèles avec modèles différents OK + après redémarrage de l'app, les sessions et leurs messages sont restaurés automatiquement. Boutons ✕ et Fork opérationnels.

## J3 — Outils agent + permissions  ✅
- Module `tools/` : trait `Tool` + `ToolRegistry` + 6 implémentations built-in
  - `read_file` (lecture, sandboxée via `sandbox_resolve`)
  - `write_file` (création/écrasement, mkdir parents auto)
  - `edit_file` (remplacement exact unique — refuse les multiples)
  - `glob` (pattern `**/*.rs` via `globwalk`)
  - `grep` (regex + `walkdir`, skip `node_modules`/`target`/`dist`/`.git`/dotfiles)
  - `bash` (`cmd /C` sur Windows, `/bin/sh -c` ailleurs, timeout 30s)
  - Sandboxing workspace-rooted : refus de tout chemin qui remonte hors du workspace
- Module `permissions/` : `Gateway` async avec `oneshot` + `Policy` (Auto/Ask/Deny)
  - Defaults prudents : read/glob/grep = Auto, write/edit/bash = Ask
  - Event `permission:request` pour dialogue UI ; `permission_respond` IPC
  - Prévisualisation lisible des arguments (`→ path`, `$ command`)
- Boucle d'agent `SessionManager::agent_loop` : LLM → tool_calls → permission → exec → message `tool` → re-LLM
  - Borne `MAX_TOOL_ITERATIONS` = 32 pour prévenir les dérives infinies
  - Events `session:tool_call` et `session:tool_result` pour l'UI
- `AGENTS.md` (à la racine du workspace) injecté comme premier message `system`
  - Convention reprise d'Opencode → consignes de style/sécurité/architecture
- Provider Ollama : envoi du `tools` body (function-calling natif Ollama)
  - Parsing `tool_calls` des chunks NDJSON → `ChatEvent::ToolCall`
  - Compatible models tool-use : Llama 3.1+, Qwen 2.5, Gemma 4, Mistral Nemo…
- UI :
  - Modale `PermissionDialog` : preview + arguments JSON + Allow/Deny
  - Tool-call blocks inline dans le chat (pending/ok/refusé, with résultat repliable)
- Tests Rust : 5/5 (catalogue, AGENTS.md absent/présent, import_custom)
- **DoD** : l'agent peut lire, modifier et exécuter des commandes dans le workspace après approbation utilisateur.

## J4 — Modèles distants (catalogue + downloader)  ✅
- **Garde-fou hardware** : module `hardware/` détecte RAM totale / CPU / OS / arch via `sysinfo`, **VRAM GPU dédiée** (DXGI sur Windows, sysfs sur Linux, system_profiler sur macOS). Relaxation de `can_run_model` quand le modèle tient en VRAM. ✅ (J1.5)
- **Parser catalogue TOML embarqué** (`docs/models-catalog.toml` via `include_str!`), exposition IPC `model_catalog_list` + UI Catalogue avec badges éligibilité et tags Ollama. ✅ (J1.5)
- **Downloader async robuste** :
  - HTTP `Range: bytes=<n>-` pour reprise sur interruption
  - SHA256 incrémental `sha2` (vérifié contre `entry.sha256` sauf si `TODO_J4`)
  - `CancellationToken` partagé via `DownloadManager` (pause/cancel sans perte)
  - Throttle events 200 ms (anti-spam IPC)
  - Écrit `.part` puis renomme à la fin si hash OK
  - Gestion 200 vs 206 (serveur ignore Range) avec truncate `.part` si recommencé
  - 3 events Tauri : `model:download:progress` / `done` / `error`
- **Registry persistant** `~/.cyonima/models/registry.json` :
  - Arc<RwLock<RegistryFile>> partagé via AppState
  - Flush atomique (temp + rename) pour éviter corruption sur crash
  - `list_installed` réel renvoyant les modèles avec `installed_path`
  - Tests : 9/9 (catalogue, registry upsert/list/remove, downloader manager init)
- **IPC** : `model_download(model_id)` async, spawn tokio::task et retourne immédiatement après garde-fou hardware
- **UI CatalogView** :
  - Boutons Télécharger / Annuler / Réessayer / Bloqué (selon éligibilité + état)
  - Barre de progression inline sous la ligne (largeur %, octets, vitesse Mo/s)
  - Ligne d'erreur repliable
  - Bouton "Rafraîchir" pour recharger le catalogue après install
- **UI StatusBar** : "N téléchargements en cours" quand actifs
- **DoD J4** : un clic sur Télécharger lance un download robuste, repreneable sur cancel/kill, vérifié en SHA256, enregistré dans le registry. Tente `gemma-4-e2b-it-qat-q4_0` (taille 3.4 Go) pour tester.

### J4.5 — Backend candle GGUF built-in (à venir)
- Câbler `LlamaCppProvider` via `candle-core` + `candle-transformers` pour l'inférence 100% offline sans dépendre d'Ollama.
- Maintenant testable contre un vrai GGUF téléchargé via J4.

## J5 — Import modèles entreprise  ✅
- UI "Importer un modèle" avec file picker natif (Tauri dialog plugin)  ✅
- Enregistrement metadata + chemin dans registry  ✅ (validate_custom + IPC model_import_custom)
- Détection automatique des GGUF (filtre *.gguf)  ✅
- **DoD** : un GGUF tiers Windows est utilisable après import.  ✅

## J6 — API distantes  ✅
- `OpenAIProvider`, `AnthropicProvider`, `GeminiProvider`, `OpenAICompatProvider`  ✅
- IPC `provider_set_api_key` / `get` / `has` / `delete` / `list_configured`  ✅
- Settings UI pour provider + clé (keyring OS)  ✅
- **DoD** : chat avec GPT-4o, Claude 3.5 et Gemini Pro via clés utilisateur.  ✅

## J7 — Ollama provider  ✅
- Détection auto (`http://localhost:11434`)  ✅ (déjà dans OllamaProvider)
- IPC `ollama_list_models` (`GET /api/tags`) + `ollama_pull_model` (`POST /api/pull` streaming)  ✅
- UI OllamaView : modèles installés, pull populaire (grille), pull custom, progression temps réel  ✅
- **DoD** : un modèle Ollama local est utilisable sans redownload.  ✅

## J8 — Recherche sémantique + indexing
- Intégration de l'embedder local embarqué
- Index SQLite + embeddings pour le workspace
- Outil `semantic_search` pour l'agent
- **DoD** : l'agent peut chercher "où est géré le panier dans le code" et obtenir des hits pertinents.

## J9 — Settings + config par projet  ✅
- UI settings complète (providers, storage, permissions)  ✅
- `ConfigManager` : TOML global `~/.cyonima/config.toml` + override `<workspace>/.cyonima/config.toml`  ✅
- Merge global/workspace avec priorité workspace  ✅
- IPC `config_get`, `config_get_workspace`, `config_set_*`  ✅
- UI `ConfigView` : provider/modèle par défaut, endpoint Ollama, permissions par outil  ✅
- 3 tests Rust : default config, set/get provider, merge workspace  ✅
- **DoD** : un workspace peut choisir son modèle + ses permissions propres.  ✅

## J10 — Polissage UI
- Diff viewer + apply/reject
- Syntax highlight (shiki)
- Thèmes (clair/sombre/contrasté)
- Raccourcis clavier, onboarding, multi-window
- **DoD** : snappy et utilisable sans doc.

## J11 — Packaging & release  ✅ (automatisation en place)
- **Workflow `release.yml`** (déclenché par un tag `v*`) via `tauri-apps/tauri-action` :
  - Build matriciel : Windows, Linux (Ubuntu 22.04), macOS Apple Silicon + Intel
  - Création automatique d'une **Release GitHub** (en brouillon) avec les installateurs
    joints : `.msi` (Windows), `.dmg` (macOS ×2), `.deb` + `.AppImage` (Linux)
  - Job `checksums` : génère et publie `SHA256SUMS.txt` sur la Release
- **Signature de code** : le workflow consomme les secrets s'ils sont présents
  (macOS : `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`,
  `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` pour la notarization ; Windows :
  à câbler via Azure Trusted Signing dans `tauri.conf.json`). Sans secrets, les
  installateurs sont produits **non signés** (fonctionnels).
- **Changelog** : `CHANGELOG.md` (Keep a Changelog / SemVer), entrée v1.0.0.
- **Version** : `1.0.0` (package.json, Cargo.toml, tauri.conf.json, status bar).
- **DoD** : release publique v1.0.0 sur les 3 OS.
  - Reste à faire côté mainteneur : fournir les certificats de signature (secrets),
    pousser le tag `v1.0.0` pour déclencher le build, relire puis publier le brouillon
    de Release. (Store description / site : hors périmètre code.)