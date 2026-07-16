# Roadmap — Cyonima-code-agent v1

Les jalons sont prévus pour être livrés dans l'ordre. Chacun a des critères de définition (DoD) clairs.

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

## J2 — Multi-session
- `SessionManager` : pool, fork, persistance SQLite
- UI : onglets multiples, état par session
- Event routing par `session_id`
- **DoD** : 2 sessions parallèles avec modèles différents OK.

## J3 — Outils agent + permissions
- Outils `read_file`, `write_file`, `edit_file`, `glob`, `grep`, `bash`
- Gateway permissions + dialogue UI
- AGENTS.md injecté dans le system prompt
- `permission_respond` IPC
- **DoD** : l'agent peut modifier un fichier du workspace après approbation.

## J4 — Modèles distants (catalogue + downloader)
- **Garde-fou hardware** : module `hardware/` détecte RAM totale / CPU / OS / arch via `sysinfo`, **VRAM GPU dédiée** (DXGI sur Windows, sysfs sur Linux, system_profiler sur macOS). Relaxation de `can_run_model` quand le modèle tient en VRAM. ✅ (J1.5)
- **Parser catalogue TOML embarqué** (`docs/models-catalog.toml` via `include_str!`), exposition IPC `model_catalog_list` + UI Catalogue avec badges éligibilité et tags Ollama. ✅ (J1.5)
- ⏳ Downloader async (HTTP range + SHA256 + reprise + pause) + événements `model:download:progress`
- ⏳ UI progression téléchargement + vérif espace disque
- **DoD** : téléchargement Qwen2.5-Coder-7B Q4 depuis HuggingFace depuis l'UI.

## J5 — Import modèles entreprise
- UI "Importer un modèle"
- Enregistrement metadata + chemin dans registry
- Détection automatique des GGUF
- **DoD** : un GGUF tiers Windows est utilisable après import.

## J6 — API distantes
- `OpenAIProvider`, `AnthropicProvider`, `GeminiProvider`, `OpenAICompatProvider`
- Settings UI pour provider + clé (keyring)
- **DoD** : chat avec GPT-4o, Claude 3.5 et Gemini Pro via clés utilisateur.

## J7 — Ollama provider
- Détection auto (`http://localhost:11434`)
- Pull/liste des modèles déjà installés côté Ollama
- **DoD** : un modèle Ollama local est utilisable sans redownload.

## J8 — Recherche sémantique + indexing
- Intégration de l'embedder local embarqué
- Index SQLite + embeddings pour le workspace
- Outil `semantic_search` pour l'agent
- **DoD** : l'agent peut chercher "où est géré le panier dans le code" et obtenir des hits pertinents.

## J9 — Settings + config par projet
- UI settings complète (providers, storage, permissions, thème)
- Override `.cyonima/config.toml` par projet
- Migration de schéma de config
- **DoD** : un workspace peut choisir son modèle + ses permissions propres.

## J10 — Polissage UI
- Diff viewer + apply/reject
- Syntax highlight (shiki)
- Thèmes (clair/sombre/contrasté)
- Raccourcis clavier, onboarding, multi-window
- **DoD** : snappy et utilisable sans doc.

## J11 — Packaging & release
- Signe macOS (notarization), Windows (sigstore via Trusted Signing)
- Installateurs : `.msi`, `.dmg`, `.deb`, `.AppImage`
- GitHub Releases auto + checksums
- Changelog, store description, site
- **DoD** : release publique v1.0.0 sur les 3 OS.