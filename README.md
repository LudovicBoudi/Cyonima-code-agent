# Cyonima-ia-code-agent

> Agent IA de code **100% local, gratuit et open source** pour Windows, macOS et Linux.
> Multi-session, multi-modèles, multi-backends. Inspiré d'[Opencode](https://opencode.ai/go?ref=ZB69DAJS6H) et de [Kiro IDE](https://kiro.dev).
> **Cyonima** est une marque.

## Pourquoi Cyonima IA ?

La majorité des outils d'IA « locaux » existants limitent leur usage pour forcer un abonnement.
Cyonima-ia-code-agent est l'inverse :

- **Gratuit pour toujours** : aucune télémétrie, aucun compte, aucune session distante obligatoire.
- **Open source (MIT)** : code, modèles par défaut, et catalogue de téléchargement tous ouverts.
- **Vraiment local** : l'inférence tourne sur votre machine via `llama.cpp` (intégré), `Ollama` (externe) ou des API distantes si vous le souhaitez.
- **Multi-session parallèle** : lancez plusieurs agents concurrents sur le même projet, chacun avec son propre modèle, son propre contexte et ses propres outils — à la Opencode.
- **Modèles respectant la taille de repo GitHub < 50 Mo** :
  - un micro-embedder < 50 Mo est embarqué pour la recherche sémantique zero-config ;
  - tous les LLM (Qwen, Llama, Gemma, Mistral, DeepSeek, etc.) sont **téléchargés à la demande** depuis HuggingFace / Ollama Library ;
  - les entreprises peuvent **importer leurs propres GGUF** en un clic.

## Stack technique

| Couche | Choix |
|---|---|
| Backend | Rust + [Tauri 2](https://tauri.app) |
| Inférence locale | bindings `llama.cpp` (built-in) + HTTP Ollama optionnel |
| API distantes | OpenAI, Anthropic, Gemini, OpenAI-compatible (LM Studio, vLLM, endpoints d'entreprise) |
| Frontend | React + TypeScript + Vite + TailwindCSS |
| Recherche sémantique | embedder GGUF local < 50 Mo + index SQLite |
| Sécurité | `keyring` OS pour stocker les clés API (DPAPI / Keychain / Secret Service) |
| Licence | MIT |

## Installation

### Prérequis

- **Rust** ≥ 1.77 ([rustup](https://rustup.rs))
- **Node.js** ≥ 20
- **Tauri 2 prerequisites** : WebView2 (Windows), Xcode CLT (macOS), `webkit2gtk` (Linux)

### Build & dev

```bash
npm install
npm run tauri dev
```

### Release build

```bash
npm run tauri build
```

---

## Sources de modèles

Cyonima gère trois sources :

| Source | Comment ça marche | Idéal pour |
|---|---|---|
| **Catalogue intégré** | Téléchargement en 1 clic depuis l'app. Le garde-fou hardware vérifie RAM + VRAM avant de lancer le téléchargement. | Découvrir et tester les modèles. |
| **Ollama local** | `ollama pull <tag>`, puis créer une session avec le provider `ollama` et le tag. | Utiliser les modèles déjà installés côté Ollama. |
| **Import custom** | Pointer vers n'importe quel `.gguf` local via le menu « Importer un modèle ». | Modèles internes entreprise, modèles non listés. |

> Stockage configurable : défaut `~/.cyonima/models/`, modifiable en J9 (Settings global) ou `.cyonima/config.toml` (par projet).

---

## Choisir son modèle

### Par hardware disponible

| Cible | RAM | GPU (VRAM) | Modèles recommandés | Lien catalogue |
|---|---|---|---|---|
| **Laptop / mobile** | 4–8 Go | 0–4 Go | Gemma 4 E2B, Qwen 2.5 3B, Llama 3.2 3B | Entry-level |
| **Laptop pro / desktop** | 16 Go | 6–8 Go | Gemma 4 12B, Qwen Coder 7B, Phi-4 14B | Mid-range |
| **Workstation** | 32 Go | 12–16 Go | Gemma 4 26B-A4B, Qwen Coder 32B, Gemma 3 27B | High-end |
| **Serveur / GPU lourd** | 64+ Go | 24+ Go | Llama 3.1 70B, Gemma 4 31B, DeepSeek R1 32B | Server |

> **Règle simple** : si le modèle tient entièrement en VRAM GPU, la RAM CPU est très peu utilisée et l'inférence est 3–10× plus rapide. Le garde-fou Cyonima détecte automatiquement la VRAM et ajuste les seuils.

### Par cas d'usage

| Besoin | Modèle conseillé | Pourquoi |
|---|---|---|
| **Coding agent local (défaut)** | Gemma 4 12B IT QAT | Bon equilibre vitesse/qualité, vision, 256K context, function-calling natif, Apache-2.0. |
| **Coding budget (4–8 Go RAM)** | Qwen 2.5 Coder 7B | Spécialisé code, 92 langages, FIM, Apache-2.0, rapide sur CPU. |
| **Coding haute qualité** | Qwen 2.5 Coder 32B | SOTA open-source Apache pour le code. Demande 24+ Go RAM. |
| **Raisonnement / math** | DeepSeek R1 Distill Qwen 14B | Mode « thinking » long, MIT. Excellent en problem-solving. |
| **Vision (screenshots, UI, OCR)** | Gemma 4 26B-A4B | MoE léger à l'inférence, vision+texte 256K. Ou Gemma 3 12B pour du plus léger. |
| **Edge / mobile** | Gemma 4 E2B | 2.3B params, vision+audio, 128K context, 4 Go RAM. |
| **Multilingue** | Qwen 2.5 14B | Fort en chinois, anglais, et langues secondaires. Apache-2.0. |
| **Rapide sur CPU pur (pas de GPU)** | Gemma 4 E4B ou Llama 3.2 3B | Petit, peu de RAM, inférable sur tout hardware. |

### Par type de provider

| Provider | Avantages | Inconvénients | Configuration |
|---|---|---|---|
| **llama.cpp (built-in)** | 100% local, aucun service externe, GGUF natif | Pas encore câblé (J4.5 prévu) | Aucune — c'est le backend par défaut. |
| **Ollama** | Large catalogue, `ollama pull` simple, outil mature | Nécessite Ollama installé séparément | Endpoint `localhost:11434` détecté auto. |
| **OpenAI** | GPT-4o, GPT-4.1 — très fort en code et instruction-following | Clé API + coût token | Settings > API Keys. |
| **Anthropic** | Claude Opus/Sonnet — excellent en reasoning long context | Clé API + coût token | Settings > API Keys. |
| **Gemini** | Gemini 2.5 Pro — 1M context, vision, gratuit en tier gratuit | Clé API Google | Settings > API Keys. |
| **OpenAI-compat** | LM Studio, vLLM, TGI, endpoints entreprise — même format que OpenAI | Dépend du serveur lancé | Endpoint URL configurable (ex: `http://localhost:1234/v1`). |

---

## Catalogue de modèles — référence complète

Toutes les tailles sont approximatives. Les valeurs RAM sont pour l'inférence CPU ; avec un GPU, les besoins RAM sont souvent réduits.

### Gemma 4 — multimodal texte+image+audio, 128–256K context, function-calling, Apache-2.0

**Recommandé pour le coding agent local.** Les tags Ollama permettent un usage direct : `ollama pull <tag>`.

| Modèle | Params | Quant. | Taille GGUF | RAM min | VRAM idéale | Tag Ollama | Usage |
|---|---|---|---|---|---|---|---|
| Gemma 4 E2B IT | 2.3B | Q4_0 QAT | 3.4 Go | 4 Go | 4 Go | `gemma4:e2b` | Edge, mobile, laptop faible. Vision+audio, 128K. |
| Gemma 4 E4B IT | 4.5B | Q4_0 QAT | 5.2 Go | 6 Go | 6 Go | `gemma4:e4b` | Laptop standard. Vision+audio 128K. Bon ratio perf/latence. |
| Gemma 4 12B IT | 12B | Q4_0 QAT | 7.0 Go | 8 Go | 8 Go | `gemma4:12b` | **Cible par défaut coding agent.** Reasoning+code+vision 256K. |
| Gemma 4 26B A4B (MoE) | 25.2B / 3.8B actifs | Q4_0 QAT | 14.4 Go | 16 Go | 16 Go | `gemma4:26b` | MoE : léger à l'inférence malgré le poids. Vision 256K. Excellent long-context. |
| Gemma 4 31B IT | 31B | Q4_0 QAT | 17.7 Go | 21 Go | 16 Go | `gemma4:31b` | Dense frontier. Codeforces ELO 2150. Demande 24+ Go RAM idéalement. |

### Gemma 3 — multimodal texte+image, 128K context

 Licence Gemma Terms (Acceptable Use Policy — usage non-commercial et commercial autorisé sous conditions).

| Modèle | Quant. | Taille | RAM min | VRAM idéale | Usage |
|---|---|---|---|---|---|
| Gemma 3 4B IT | Q4_K_M | ~2.5 Go | 6 Go | 4 Go | Petit multimodal, bon pour petits workspaces. |
| Gemma 3 4B IT | Q5_K_M | ~2.8 Go | 6 Go | 4 Go | Vari qualité supérieure. |
| Gemma 3 4B IT | Q8_0 | ~4.3 Go | 8 Go | 4 Go | Haute fidélité. |
| Gemma 3 12B IT | Q4_K_M | ~7.5 Go | 12 Go | 8 Go | Multimodal full-featured. |
| Gemma 3 12B IT | Q5_K_M | ~9 Go | 14 Go | 8 Go | Vari qualité supérieure. |
| Gemma 3 12B IT | Q8_0 | ~14 Go | 16 Go | 12 Go | Haute fidélité. |
| Gemma 3 27B IT | Q4_K_M | ~16 Go | 20 Go | 16 Go | Haut de gamme, similaire à Gemma 4 E4B mais plus dense. |
| Gemma 3 27B IT | Q5_K_M | ~18 Go | 24 Go | 16 Go | Vari qualité supérieure. |

### Coding spécialisés — Qwen 2.5 Coder

Les modèles Coder sont entraînés spécifiquement pour la génération de code (FIM + instruction-following). Apache-2.0.

| Modèle | Quant. | Taille | RAM min | VRAM idéale | Usage |
|---|---|---|---|---|---|
| Qwen 2.5 Coder 7B | Q4_K_M | ~4.5 Go | 8 Go | 6 Go | **Coding budget.** 92 langages, FIM, bon démarrage. |
| Qwen 2.5 Coder 14B | Q4_K_M | ~9 Go | 12 Go | 10 Go | Meilleure qualité, 128K context, idéal code complexe. |
| Qwen 2.5 Coder 32B | Q4_K_M | ~20 Go | 24 Go | 16 Go | **SOTA coding open-source Apache.** Demande workstation. |

### Reasoning — DeepSeek R1 distillations

Mode « thinking » long : le modèle raisonne étape par étape avant de répondre. MIT license.

| Modèle | Quant. | Taille | RAM min | VRAM idéale | Usage |
|---|---|---|---|---|---|
| DeepSeek R1 Distill Qwen 1.5B | Q4_K_M | ~1 Go | 2 Go | 2 Go | Tests reasoning peu coûteux. |
| DeepSeek R1 Distill Llama 8B | Q4_K_M | ~4.5 Go | 8 Go | 6 Go | Reasoning milieu de gamme. |
| DeepSeek R1 Distill Qwen 14B | Q4_K_M | ~9 Go | 12 Go | 10 Go | Reasoning qualité supérieure. |
| DeepSeek R1 Distill Qwen 32B | Q4_K_M | ~20 Go | 24 Go | 16 Go | Reasoning lourd. Demande workstation. |

### Généralistes — Llama / Mistral / Phi / Qwen

Polyvalents : conversation, instruction-following, code occasionnel.

| Modèle | Licence | Quant. | Taille | RAM min | VRAM idéale | Usage |
|---|---|---|---|---|---|---|
| Llama 3.1 8B | Llama Community | Q4_K_M | ~4.5 Go | 8 Go | 6 Go | Référence generaliste, large ecosystem. |
| Llama 3.1 8B | Llama Community | Q8_0 | ~8 Go | 12 Go | 8 Go | Haute fidélité. |
| Mistral 7B v0.3 | Apache-2.0 | Q4_K_M | ~4.5 Go | 8 Go | 6 Go | Dense et efficace pour la taille. |
| Mistral Nemo 12B | Apache-2.0 | Q4_K_M | ~7.5 Go | 12 Go | 8 Go | Long context 128K. |
| Phi-4 14B | MIT | Q4_K_M | ~9 Go | 12 Go | 10 Go | Excellent raisonnement pour sa taille. |
| Phi-4 14B | MIT | Q8_0 | ~14 Go | 18 Go | 12 Go | Haute fidélité. |
| Qwen 2.5 14B | Apache-2.0 | Q4_K_M | ~9 Go | 12 Go | 10 Go | Multilingue solide. |
| Qwen 2.5 32B | Apache-2.0 | Q4_K_M | ~20 Go | 24 Go | 16 Go | Taille limite avant 70B. |

### Entry-level — 1.5B–3B

Pour les machines à faible RAM ou pour des tâches simples (chat, résumé, extraction).

| Modèle | Licence | Quant. | Taille | RAM min | Usage |
|---|---|---|---|---|---|
| Qwen 2.5 3B | Apache-2.0 | Q4_K_M | ~2 Go | 4 Go | Démarrage rapide. |
| Llama 3.2 3B | Llama Community | Q4_K_M | ~2 Go | 4 Go | Démarrage rapide. |
| DeepSeek R1 Distill Qwen 1.5B | MIT | Q4_K_M | ~1 Go | 2 Go | Ultra-léger, reasoning basique. |

### Multimodal vision+texte — Qwen 2.5 VL

Pour analyser des images, screenshots, documents scannés, UI. Apache-2.0.

| Modèle | Quant. | Taille | RAM min | VRAM idéale | Usage |
|---|---|---|---|---|---|
| Qwen 2.5 VL 3B | Q4_K_M | ~2 Go | 6 Go | 4 Go | Vision budget. |
| Qwen 2.5 VL 7B | Q4_K_M | ~4.5 Go | 8 Go | 6 Go | Vision standard. |
| Qwen 2.5 VL 32B | Q4_K_M | ~20 Go | 24 Go | 16 Go | Vision lourde, OCR/screenshots complexes. |

### High-end 70B

Pour les serveurs ou workstations avec beaucoup de RAM/VRAM. Lent sans GPU dédié.

| Modèle | Quant. | Taille | RAM min | VRAM idéale | Usage |
|---|---|---|---|---|---|
| Llama 3.1 70B | Q2_K | ~26 Go | 32 Go | 16 Go | Quantizé bas pour tenir en RAM. Lent sans gros GPU. |
| Llama 3.1 70B | Q3_K_M | ~32 Go | 40 Go | 24 Go | Demande serveur ou GPU 24+ Go VRAM. |

### Embedders — recherche sémantique

Pour la recherche dans le code (RAG) et l'outil `semantic_search` de l'agent.

| Modèle | Quant. | Taille | RAM min | Usage |
|---|---|---|---|---|
| all-MiniLM-L6-v2 (embarqué) | Q8 | ~23 Mo | 1 Go | **Par défaut.** Zero-config, inclus dans le repo. |
| BGE Small EN v1.5 | Q8_0 | ~35 Mo | 1 Go | Alternative légère. |
| Nomic Embed Text v2 MoE | Q8_0 | ~600 Mo | 2 Go | Long context 2K, vocabulaire plus large. |

---

## Notes importantes

### Licences

Chaque modèle conserve sa licence d'origine. Avant de télécharger, Cyonima affiche la licence dans le catalogue. Résumé :

| Licence | Usage commercial | Restrictions notables |
|---|---|---|
| Apache-2.0 | Oui | Aucune restriction majeure. (Gemma 4, Qwen, Mistral) |
| MIT | Oui | Aucune restriction. (DeepSeek R1, Phi-4, BGE) |
| Gemma Terms (AUP) | Oui (sous conditions) | Usage responsable requis, pas de armes/surveillance. (Gemma 3) |
| Llama Community | Oui (> 700M mois actifs) | License Meta, threshold d'usage commercial. |

### Garde-fou hardware

Le downloader vérifie avant chaque téléchargement :
1. **RAM totale** vs `ram_min_gb` du modèle (marge de 1 Go pour l'OS)
2. **VRAM GPU** (DXGI sur Windows, sysfs sur Linux, system_profiler sur macOS)
3. Si le modèle tient **entièrement en VRAM**, la contrainte RAM est relaxée

Le garde-fou est aussi exposé en IPC : `hardware_get()` et `hardware_can_run_model(ram_min_gb)`.

### API distantes

Pour les providers distants (OpenAI, Anthropic, Gemini), les clés API sont stockées dans le keyring OS (DPAPI / Keychain / Secret Service) — jamais en clair sur disque. Configuration via Settings dans l'app.

---

## Documentation

- [`ARCHITECTURE.md`](docs/ARCHITECTURE.md) — schémas de l'application et des modules Rust
- [`ROADMAP.md`](docs/ROADMAP.md) — jalons J0 → J11
- [`AGENTS.md`](AGENTS.md) — instructions par défaut de l'agent Cyonima sur ce repo
- [`docs/models-catalog.toml`](docs/models-catalog.toml) — catalogue de modèles téléchargeables

## Licence

MIT — voir [`LICENSE`](LICENSE).

Les modèles téléchargés conservent leurs propres licences qui sont affichées avant tout téléchargement dans l'application.
