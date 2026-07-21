# Cyonima-ia-code-agent

> Agent IA de code **100% local, gratuit et open source** pour Windows, macOS et Linux.
> Multi-session, propulsé par [Ollama](https://ollama.com). Inspiré d'[Opencode](https://opencode.ai/go?ref=ZB69DAJS6H) et de [Kiro IDE](https://kiro.dev).

## Pourquoi ?

La majorité des outils d'IA « locaux » existants limitent leur usage pour forcer un abonnement.
Cyonima-ia-code-agent est l'inverse :

- **Gratuit pour toujours** : aucune télémétrie, aucun compte, aucune session distante.
- **Open source (MIT)** : code et catalogue tous ouverts.
- **Vraiment local** : l'inférence tourne sur votre machine via [Ollama](https://ollama.com). Rien ne sort de votre poste.
- **Multi-session parallèle** : lancez plusieurs agents concurrents sur le même projet, chacun avec son propre modèle, son propre contexte et ses propres outils — à la Opencode.
- **Zéro gros fichier dans le repo** : les modèles sont gérés par Ollama (`ollama pull`), jamais committés dans Git.

## Stack technique

| Couche | Choix |
|---|---|
| Backend | Rust + [Tauri 2](https://tauri.app) |
| Inférence | [Ollama](https://ollama.com) via HTTP local (`localhost:11434`) |
| Frontend | React + TypeScript + Vite + TailwindCSS |
| Persistance | SQLite (sessions + messages) |
| Licence | MIT |

## Installation

### Télécharger (recommandé)

Récupérez l'installateur pour votre OS depuis la page
[**Releases**](https://github.com/LudovicBoudi/Cyonima-code-agent/releases) :

- **Windows** : `.msi`
- **macOS** : `.dmg` (Apple Silicon ou Intel)
- **Linux** : `.deb` ou `.AppImage`

Vérifiez l'intégrité via `SHA256SUMS.txt` joint à la release. Dans tous les cas,
[Ollama](https://ollama.com) doit être installé et lancé (`ollama serve`).

### Prérequis (build depuis les sources)

- **[Ollama](https://ollama.com)** installé et lancé (`ollama serve`) — c'est le moteur d'inférence.
- **Rust** ≥ 1.77 ([rustup](https://rustup.rs))
- **Node.js** ≥ 20
- **Tauri 2 prerequisites** : WebView2 (Windows), Xcode CLT (macOS), `webkit2gtk` (Linux)

## Démarrage rapide

```bash
# 1. Installer Ollama puis récupérer au moins un modèle
ollama pull qwen2.5-coder:7b

# 2. Lancer l'app en dev
npm install
npm run tauri dev
```

Dans l'application :

1. **Nouvelle session** → choisissez le répertoire de travail (c'est la seule chose demandée).
2. Dans le chat, choisissez le modèle dans le **menu déroulant**, parmi ceux installés dans Ollama.
3. Discutez : l'agent peut lire, modifier et exécuter des commandes dans le workspace (après approbation).

### Release build

```bash
npm run tauri build
```

---

## Interface

Thème **violet** unique (sombre). La vue session est organisée en **deux colonnes** :

- **Gauche (75%)** — la conversation : raisonnement du modèle (bloc repliable), réponses, appels d'outils, puis la **chatbox**. La chatbox propose :
  - le **sélecteur de modèle** (modèles Ollama installés) ;
  - un menu d'**intensité de raisonnement** (Auto / Désactivé / Faible / Moyen / Élevé) pour les modèles « thinking » ;
  - un **indicateur d'usage de contexte** (tokens du dernier tour vs taille de contexte du modèle) ;
  - les boutons **Play / Stop** pour envoyer ou interrompre.
- **Droite (25%)** — les **fichiers modifiés** du workspace (ajoutés / modifiés / supprimés / renommés), via `git status`. Les répertoires de travail sont supposés être des dépôts git.

---

## Gérer ses modèles

Tous les modèles passent par Ollama :

- **Onglet Ollama** de l'app : voir les modèles installés, lancer un `pull` avec suivi de progression.
- **CLI** : `ollama pull <tag>` (ex: `ollama pull deepseek-r1:14b`).

Le menu déroulant du chat liste automatiquement les modèles présents dans Ollama. Un garde-fou hardware (RAM / VRAM) vous indique si un modèle est adapté à votre machine.

> Le catalogue de tags suggérés vit dans [`docs/models-catalog.toml`](docs/models-catalog.toml).

---

## Choisir son modèle

### Par hardware disponible

| Cible | RAM | GPU (VRAM) | Tags Ollama recommandés |
|---|---|---|---|
| **Laptop / faible RAM** | 4–8 Go | 0–4 Go | `llama3.2:3b`, `qwen2.5:3b`, `gemma3:4b` |
| **Laptop pro / desktop** | 16 Go | 6–8 Go | `qwen2.5-coder:7b`, `gemma3:12b`, `phi4:14b` |
| **Workstation** | 32 Go | 12–16 Go | `qwen2.5-coder:32b`, `deepseek-r1:32b`, `gemma3:27b` |
| **Serveur / GPU lourd** | 64+ Go | 24+ Go | `llama3.1:70b`, `qwen2.5:72b` |

> **Règle simple** : si le modèle tient entièrement en VRAM GPU, la RAM CPU est très peu utilisée et l'inférence est 3–10× plus rapide. Le garde-fou Cyonima détecte automatiquement la VRAM et ajuste les seuils.

### Par cas d'usage

| Besoin | Tag Ollama conseillé | Pourquoi |
|---|---|---|
| **Coding agent (défaut)** | `qwen2.5-coder:7b` | Spécialisé code, 92 langages, rapide, function-calling. |
| **Coding haute qualité** | `qwen2.5-coder:32b` | SOTA open-source pour le code. Demande 24+ Go RAM. |
| **Raisonnement / math** | `deepseek-r1:14b` | Mode « thinking » long, affiché dans un bloc repliable. |
| **Généraliste léger** | `llama3.2:3b` | Démarrage rapide, peu de RAM. |
| **Généraliste équilibré** | `gemma3:12b` ou `qwen2.5:14b` | Bon compromis qualité / vitesse. |
| **Multilingue** | `qwen2.5:14b` | Fort en anglais, chinois et langues secondaires. |

> Les modèles « thinking » (DeepSeek-R1, Qwen3…) affichent leur raisonnement dans un bloc « Raisonnement du modèle » repliable, séparé de la réponse finale. L'application active automatiquement le mode thinking uniquement pour les modèles qui le supportent.

---

## Garde-fou hardware

Avant de suggérer / lancer un modèle, l'application vérifie :

1. **RAM totale** vs RAM minimale recommandée du modèle (marge de 1 Go pour l'OS)
2. **VRAM GPU** (DXGI sur Windows, sysfs sur Linux, `system_profiler` sur macOS)
3. Si le modèle tient **entièrement en VRAM**, la contrainte RAM est relaxée

Le garde-fou est exposé en IPC : `hardware_get()` et `hardware_can_run_model(ram_min_gb)`.

## Confidentialité

- **Aucune télémétrie**, aucune donnée sortante non sollicitée.
- Toute l'inférence se fait en local via Ollama (`localhost:11434`).
- Les sessions et messages sont stockés localement en SQLite (`~/.cyonima/sessions.db`).

---

## Documentation

- [`CHANGELOG.md`](CHANGELOG.md) — historique des versions
- [`ARCHITECTURE.md`](docs/ARCHITECTURE.md) — schémas de l'application et des modules Rust
- [`ROADMAP.md`](docs/ROADMAP.md) — jalons de développement
- [`models-guide.md`](docs/models-guide.md) — quel modèle choisir selon la tâche (comparatif + repères)
- [`AGENTS.md`](AGENTS.md) — instructions par défaut de l'agent Cyonima sur ce repo
- [`docs/models-catalog.toml`](docs/models-catalog.toml) — catalogue de tags Ollama suggérés

## Licence

MIT — voir [`LICENSE`](LICENSE).

Les modèles utilisés via Ollama conservent leurs propres licences (affichées dans le catalogue avant utilisation).
