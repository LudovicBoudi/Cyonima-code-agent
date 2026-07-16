# Cyonima-code-agent

> Agent IA de code **100% local, gratuit et open source** pour Windows, macOS et Linux.
> Multi-session, multi-modèles, multi-backends. Inspiré d'[Opencode](https://opencode.ai/go?ref=ZB69DAJS6H) et de [Kiro IDE](https://kiro.dev).

## Pourquoi Cyonima ?

La majorité des outils d'IA « locaux » existants limitent leur usage pour forcer un abonnement.
Cyonima-code-agent est l'inverse :

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
| Frontend | React + TypeScript + Vite + TailwindCSS + shadcn/ui |
| Recherche sémantique | embedder GGUF local < 50 Mo + index SQLite |
| Sécurité | `keyring` OS pour stocker les clés API (DPAPI / Keychain / Secret Service) |
| Licence | MIT |

## Installation ( après J0 )

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

## Modèles

Cyonima gère trois sources de modèles :

1. **Embarqués (< 50 Mo)** : inclus dans le repo, disponibles immédiatement au premier lancement.
2. **Téléchargeables** : choisis dans le catalogue intégré, téléchargés à la demande via l'app. Si un tag Ollama existe, vous pouvez aussi faire `ollama pull <tag>` puis utiliser le modèle directement via le provider Ollama.
3. **Import custom** : pointez vers n'importe quel `.gguf` local (modèles internes entreprise compris), enregistré dans le registry.

> Le garde-fou hardware vérifie votre RAM totale + VRAM GPU (DXGI/sysfs/system_profiler) **avant** tout téléchargement. Si un modèle tient entièrement en VRAM, la contrainte RAM est automatiquement relaxée. Voir `Settings > Hardware` ou le panneau Catalogue dans l'app.

Stockage **configurable** : défaut `~/.cyonima/models/`, modifiable dans Settings (global) ou `.cyonima/config.toml` (par projet).

### Modèles embarqués (< 50 Mo)

| ID | Taille | Licence | Usage |
|---|---|---|---|
| `all-MiniLM-L6-v2` Q8 | ~23 Mo | Apache-2.0 | Embedder de recherche sémantique. Permet la RAG dans le workspace et l'outil `semantic_search` de l'agent. Zero-config. |

### Gemma 4 — multimodal texte+image (+audio pour E2B/E4B), 128K-256K context, function-calling natif, system prompt natif

Tag Ollama indiqué — pour usage direct sans download Cyonima : `ollama pull <tag>` puis créer une session avec le provider `ollama` et le modèle `<tag>`.

| Modèle | Quant. | Taille | RAM min | VRAM conseillée | Tag Ollama | Licence | Usage |
|---|---|---|---|---|---|---|---|
| Gemma 4 E2B IT | Q4_0 QAT | 3.4 Go | 4 Go | 4 Go | `gemma4:e2b` | Apache-2.0 | Edge, mobile, laptop faible. 2.3B params effectifs, vision+audio, context 128K. Rapide. |
| Gemma 4 E4B IT | Q4_0 QAT | 5.2 Go | 6 Go | 6 Go | `gemma4:e4b` | Apache-2.0 | Edge/laptop standard. 4.5B params effectifs, vision+audio 128K. Bon ratio perf/latence. |
| Gemma 4 12B IT | Q4_0 QAT | 7.0 Go | 8 Go | 8 Go | `gemma4:12b` | Apache-2.0 | Workstation. Reasoning + code + vision 256K. Cible par défaut pour le coding agent local. |
| Gemma 4 26B A4B (MoE) | Q4_0 QAT | 14.4 Go | 16 Go | 16 Go | `gemma4:26b` | Apache-2.0 | MoE 25.2B / 3.8B actifs. Léger à l'inférence malgré le poids total, vision 256K. Excellent agarique long-context. |
| Gemma 4 31B IT (dense) | Q4_0 QAT | 17.7 Go | 21 Go | 16 Go | `gemma4:31b` | Apache-2.0 | Dense frontier reasoning + code. Codeforces ELO 2150. Demande 24+ Go RAM idéalement + 16 Go VRAM. |

### Gemma 3 — multimodal texte+image, context 128K

| Modèle | Quant. | Taille | RAM min | VRAM conseillée | Licence | Usage |
|---|---|---|---|---|---|---|
| Gemma 3 4B IT | Q4_K_M | ~2.5 Go | 6 Go | 4 Go | Gemma Terms (AUP) | Petit multimodal. Bon pour petits workspaces. |
| Gemma 3 4B IT | Q5_K_M | ~2.8 Go | 6 Go | 4 Go | Gemma Terms (AUP) | Vari qualité supérieure. |
| Gemma 3 4B IT | Q8_0 | ~4.3 Go | 8 Go | 4 Go | Gemma Terms (AUP) | Haute fidélité. |
| Gemma 3 12B IT | Q4_K_M | ~7.5 Go | 12 Go | 8 Go | Gemma Terms (AUP) | Workstation multimodal full-featured. |
| Gemma 3 12B IT | Q5_K_M | ~9 Go | 14 Go | 8 Go | Gemma Terms (AUP) | Vari qualité supérieure. |
| Gemma 3 12B IT | Q8_0 | ~14 Go | 16 Go | 12 Go | Gemma Terms (AUP) | Haute fidélité. |
| Gemma 3 27B IT | Q4_K_M | ~16 Go | 20 Go | 16 Go | Gemma Terms (AUP) | Haut de gamme, similariaire à Gemma 4 E4B mais plus dense. |
| Gemma 3 27B IT | Q5_K_M | ~18 Go | 24 Go | 16 Go | Gemma Terms (AUP) | Vari qualité supérieure. |

### Code spécialisés

| Modèle | Quant. | Taille | RAM min | VRAM conseillée | Licence | Usage |
|---|---|---|---|---|---|---|
| Qwen 2.5 Coder 7B Instruct | Q4_K_M | ~4.5 Go | 8 Go | 6 Go | Apache-2.0 | **Cible coding budget**. Bon FIM et 92 langages. Bon démarusage. |
| Qwen 2.5 Coder 14B Instruct | Q4_K_M | ~9 Go | 12 Go | 10 Go | Apache-2.0 | Meilleure qualité 7B, context 128K, idéal pour du code complexe. |
| Qwen 2.5 Coder 32B Instruct | Q4_K_M | ~20 Go | 24 Go | 16 Go | Apache-2.0 | SOTA coding open source Apache. Demande workstation 24+ Go RAM. |

### Reasoning (DeepSeek R1 distillations) — mode "thinking" long

| Modèle | Quant. | Taille | RAM min | VRAM conseillée | Licence | Usage |
|---|---|---|---|---|---|---|
| DeepSeek R1 Distill Qwen 1.5B | Q4_K_M | ~1 Go | 2 Go | 2 Go | MIT | Tests reasoning peu coûteux. |
| DeepSeek R1 Distill Llama 8B | Q4_K_M | ~4.5 Go | 8 Go | 6 Go | MIT | Reasoning moyen budget. |
| DeepSeek R1 Distill Qwen 14B | Q4_K_M | ~9 Go | 12 Go | 10 Go | MIT | Reasoning qualité supérieure. |
| DeepSeek R1 Distill Qwen 32B | Q4_K_M | ~20 Go | 24 Go | 16 Go | MIT | Reasoning lourd. Demande workstation. |

### Généralistes 7B-14B

| Modèle | Quant. | Taille | RAM min | VRAM conseillée | Licence | Usage |
|---|---|---|---|---|---|---|
| Llama 3.1 8B Instruct | Q4_K_M | ~4.5 Go | 8 Go | 6 Go | Llama Community | Référence généraliste large écosystème. |
| Llama 3.1 8B Instruct | Q8_0 | ~8 Go | 12 Go | 8 Go | Llama Community | Vari haute fidélité. |
| Mistral 7B Instruct v0.3 | Q4_K_M | ~4.5 Go | 8 Go | 6 Go | Apache-2.0 | Densité efficace pour la taille. |
| Mistral Nemo 12B Instruct | Q4_K_M | ~7.5 Go | 12 Go | 8 Go | Apache-2.0 | Long context 128K. |
| Phi-4 14B Instruct | Q4_K_M | ~9 Go | 12 Go | 10 Go | MIT | Pour sa taille excellent en raisonnement. |
| Phi-4 14B Instruct | Q8_0 | ~14 Go | 18 Go | 12 Go | MIT | Vari haute fidélité. |
| Qwen 2.5 14B Instruct | Q4_K_M | ~9 Go | 12 Go | 10 Go | Apache-2.0 | Apache multilingue. |
| Qwen 2.5 32B Instruct | Q4_K_M | ~20 Go | 24 Go | 16 Go | Apache-2.0 | Apache, taille limite avant 70B. |

### Généralistes entry-level

| Modèle | Quant. | Taille | RAM min | VRAM conseillée | Licence | Usage |
|---|---|---|---|---|---|---|
| Qwen 2.5 3B Instruct | Q4_K_M | ~2 Go | 4 Go | 4 Go | Apache-2.0 | Petit démarrage rapide. |
| Llama 3.2 3B Instruct | Q4_K_M | ~2 Go | 4 Go | 4 Go | Llama Community | Petit démarrage. |

### Multimodal vision+texte

| Modèle | Quant. | Taille | RAM min | VRAM conseillée | Licence | Usage |
|---|---|---|---|---|---|---|
| Qwen 2.5 VL 3B Instruct | Q4_K_M | ~2 Go | 6 Go | 4 Go | Apache-2.0 | Vision budget. |
| Qwen 2.5 VL 7B Instruct | Q4_K_M | ~4.5 Go | 8 Go | 6 Go | Apache-2.0 | Vision standard. |
| Qwen 2.5 VL 32B Instruct | Q4_K_M | ~20 Go | 24 Go | 16 Go | Apache-2.0 | Vision heavy,огра OCR/screenshots complexes. |

### High-end 70B

| Modèle | Quant. | Taille | RAM min | VRAM conseillée | Licence | Usage |
|---|---|---|---|---|---|---|
| Llama 3.1 70B Instruct | Q2_K | ~26 Go | 32 Go | 16 Go | Llama Community | Quantizé bas (Q2) pour la taille. Lent sans gros GPU. |
| Llama 3.1 70B Instruct | Q3_K_M | ~32 Go | 40 Go | 24 Go | Llama Community | Demande gros serveur ou 24+ Go VRAM. |

### Embedders alternatifs à l'embedder embarqué

| Modèle | Quant. | Taille | RAM min | VRAM conseillée | Licence | Usage |
|---|---|---|---|---|---|---|
| BGE Small EN v1.5 | Q8_0 | ~35 Mo | 1 Go | n/a | MIT | Alternative à l'embedder embarqué. |
| Nomic Embed Text v2 MoE | Q8_0 | ~600 Mo | 2 Go | n/a | Apache-2.0 | Long context 2K, plus large vocab. |

### Récapitulatif hardware par cible

| Cible | RAM | VRAM | Modèles conseillés |
|---|---|---|---|
| **Entry** (laptop, mobile) | 4-8 Go | 0-4 Go | Gemma 4 E2B, Qwen 2.5 3B, Llama 3.2 3B, Qwen Coder 7B (Q4) |
| **Mid** (laptop pro, desktop std) | 16 Go | 6-8 Go | Gemma 4 12B, Qwen 2.5 14B, Mistral Nemo 12B, Phi-4 14B Q4 |
| **High** (workstation) | 32 Go | 12-16 Go | Gemma 4 26B-A4B, Qwen 2.5 Coder 32B, Qwen 2.5 VL 32B, Gemma 3 27B |
| **Server** | 64+ Go | 24+ Go | Llama 3.1 70B Q2/Q3, Gemma 4 31B, DeepSeek R1 distill 32B |

> NB : Toutes les valeurs sont approximatives et dépendent du runtime (`llama.cpp` via candle built-in, Ollama ou API distante). La VRAM GPU accélère considérablement l'inférence ; le CPU reste la fallback universelle.

## Documentation

- [`ARCHITECTURE.md`](docs/ARCHITECTURE.md) — schémas de l'application et des modules Rust
- [`ROADMAP.md`](docs/ROADMAP.md) — jalons J0 → J11
- [`AGENTS.md`](AGENTS.md) — instructions par défaut de l'agent Cyonima sur ce repo
- [`docs/models-catalog.toml`](docs/models-catalog.toml) — catalogue de modèles téléchargeables

## Licence

MIT — voir [`LICENSE`](LICENSE).

Les modèles téléchargés conservent leurs propres licences (Llama Community, Gemma Terms, Apache 2.0, MIT, etc.) qui sont affichées avant tout téléchargement.