# Cyonima-code-agent

> Agent IA de code **100% local, gratuit et open source** pour Windows, macOS et Linux.
> Multi-session, multi-modèles, multi-backends. Inspiré d'[Opencode](https://opencode.ai) et de [Kiro IDE](https://kiro.dev).

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

- **Embarqué (< 50 Mo, dans le repo)** : un embedder `all-MiniLM-L6-v2` quantizé pour la recherche sémantique zero-config + la RAG dans le projet.
- **Téléchargeables à la demande** : Qwen 2.5 / Qwen 3, Llama 3.x, Gemma 2/3, Mistral, DeepSeek-R1 distill, Qwen 2.5 Coder, Qwen 2.5 VL, etc.
- **Import custom** : pointez vers n'importe quel `.gguf` (modèles internes entreprise compris).

Stockage **configurable** : défaut `~/.cyonima/models/`, modifiable dans Settings (global) ou `.cyonima/config.toml` (par projet).

## Documentation

- [`ARCHITECTURE.md`](docs/ARCHITECTURE.md) — schémas de l'application et des modules Rust
- [`ROADMAP.md`](docs/ROADMAP.md) — jalons J0 → J11
- [`AGENTS.md`](AGENTS.md) — instructions par défaut de l'agent Cyonima sur ce repo
- [`docs/models-catalog.toml`](docs/models-catalog.toml) — catalogue de modèles téléchargeables

## Licence

MIT — voir [`LICENSE`](LICENSE).

Les modèles téléchargés conservent leurs propres licences (Llama Community, Gemma Terms, Apache 2.0, MIT, etc.) qui sont affichées avant tout téléchargement.