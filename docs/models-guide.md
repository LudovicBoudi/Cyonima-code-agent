# Guide des modèles — quel modèle pour quelle tâche

Point de repère pour choisir le meilleur modèle selon la tâche à accomplir.

Cyonima-ia-code-agent tourne **100% en local via Ollama**. Ce guide compare les
modèles du catalogue de l'app à deux modèles **frontier** de référence
(GLM-5.2 et DeepSeek-V4-Pro) qui servent de **plafond de qualité** — ils tournent
en cloud/datacenter, **pas en local** sur une machine normale.

> Les informations sur les modèles frontier sont reformulées pour conformité.
> Sources : [GLM-5.2 (Hugging Face)](https://huggingface.co/zai-org/GLM-5.2),
> [technology.org](https://www.technology.org/2026/07/02/glm-5-2-coding-how-good-is-it-really-2026-benchmarks/),
> [DeepSeek-V4-Pro (Hugging Face)](https://huggingface.co/deepseek-ai/DeepSeek-V4-Pro),
> [apxml](https://apxml.com/models/deepseek-v4-pro).

## Comparatif — échelle & nature

| Modèle | Params (total / actifs) | Contexte | Licence | Où ça tourne | RAM mini (local) |
|---|---|---|---|---|---|
| **DeepSeek-V4-Pro** *(réf. plafond)* | 1,6T / 49B (MoE) | ~1M tokens | MIT | Cloud / datacenter | Hors portée locale |
| **GLM-5.2** *(réf. plafond)* | 744B / ~40B (MoE) | ~1M tokens | MIT | Cloud / gros serveur | Hors portée locale |
| Llama 3.1 70B (Q2_K) | 70B dense | 128K | Llama Community | Local (lourd) | 32 Go+ |
| Qwen 2.5 Coder 32B | 32B dense | ~128K | Apache-2.0 | Local | 24 Go |
| DeepSeek-R1 Distill 32B | 32B dense | ~128K | MIT | Local | 24 Go |
| Qwen 2.5 32B | 32B dense | ~128K | Apache-2.0 | Local | 24 Go |
| Gemma « 4 » 31B | 31B dense | — | Apache-2.0 | Local | 21 Go |
| Qwen 2.5 Coder 14B | 14B dense | ~128K | Apache-2.0 | Local | 12 Go |
| Phi-4 14B | 14B dense | 16K | MIT | Local | 12 Go |
| Qwen 2.5 14B | 14B dense | ~128K | Apache-2.0 | Local | 12 Go |
| Gemma « 4 » 12B | 12B dense | — | Apache-2.0 | Local | 8 Go |
| DeepSeek-R1 Distill Llama 8B | 8B dense | ~128K | MIT | Local | 8 Go |
| Llama 3.1 8B | 8B dense | 128K | Llama Community | Local | 8 Go |
| Qwen 2.5 Coder 7B | 7B dense | ~128K | Apache-2.0 | Local | 8 Go |
| Mistral 7B v0.3 | 7B dense | 32K | Apache-2.0 | Local | 8 Go |
| Qwen 2.5 VL 7B (vision) | 7B dense | ~32K | Apache-2.0 | Local | 8 Go |
| Qwen 2.5 3B / Llama 3.2 3B | 3B dense | 32K–128K | Apache / Llama | Local | 4 Go |
| DeepSeek-R1 Distill 1.5B | 1,5B dense | ~128K | MIT | Local | 2 Go |

> Les contextes marqués `~` sont les maxima théoriques du modèle. Via Ollama, la
> fenêtre effective est souvent plafonnée par `num_ctx` (fréquemment 4K–8K par
> défaut) et par la RAM/VRAM disponible.

## Repère — quel modèle pour quelle tâche

| Tâche | Meilleur choix **local** (app) | Plancher acceptable | Plafond (cloud, réf.) |
|---|---|---|---|
| **Coding agentique, refactor multi-fichiers** | Qwen 2.5 Coder 32B | Qwen 2.5 Coder 14B | GLM-5.2 (spécialisé agentic coding) |
| **Code quotidien, autocomplétion, petits patchs** | Qwen 2.5 Coder 7B | Qwen 2.5 Coder 7B | GLM-5.2 |
| **Raisonnement, debug complexe, algo/maths** | DeepSeek-R1 Distill 32B | DeepSeek-R1 Distill 8B | DeepSeek-V4-Pro |
| **Généraliste (chat, rédaction, synthèse)** | Qwen 2.5 14B / Phi-4 14B | Llama 3.1 8B | GLM-5.2 / DeepSeek-V4-Pro |
| **Réponse rapide, machine modeste** | Llama 3.2 3B / Qwen 2.5 3B | DeepSeek-R1 1.5B | — |
| **Multilingue (FR/EN/ZH…)** | Qwen 2.5 32B / 14B | Qwen 2.5 7B | GLM-5.2 |
| **Vision (captures, images, UI)** | Qwen 2.5 VL 7B | Qwen 2.5 VL 7B | GLM-5.2 (multimodal) |
| **Très gros contexte (base de code entière)** | Qwen/Llama 128K (si RAM suffit) | — | DeepSeek-V4-Pro / GLM-5.2 (1M tokens) |

## Comment lire ce repère

- **Le local ne rivalise pas avec le frontier en qualité brute**, mais il est
  gratuit, privé et hors-ligne. GLM-5.2 et DeepSeek-V4-Pro donnent l'idée du
  plafond : si une tâche échoue avec ton meilleur modèle local (ex. Qwen Coder 32B)
  et que la qualité est vraiment critique, c'est le signe qu'il faut un modèle
  frontier (via API) — pas juste un modèle local plus gros.
- **Choisis d'abord par catégorie de tâche**, **puis** par ce que ta machine
  encaisse (colonne RAM). Le garde-fou hardware de l'app indique déjà si un modèle
  passe sur ta configuration.
- **Règle pratique** :
  - coder → famille *Coder* (Qwen 2.5 Coder)
  - réfléchir / déboguer → famille *R1* (DeepSeek-R1)
  - discuter / rédiger → généralistes (Qwen, Phi-4, Llama)
  - images → *VL* (Qwen 2.5 VL)

## Modèles frontier de référence (contexte)

- **GLM-5.2** (Z.ai, ex-Zhipu) — MoE ~744B params (~40B actifs), contexte ~1M
  tokens, licence MIT. Orienté coding agentique et tâches longues ; positionné
  parmi les meilleurs modèles open-weights de 2026.
- **DeepSeek-V4-Pro** — MoE 1,6T params (49B actifs), contexte ~1M tokens, licence
  MIT. Architecture d'attention hybride (CSA + HCA) réduisant fortement le coût
  d'inférence ; performances proches des meilleurs modèles propriétaires.

Ces deux modèles ne sont **pas installables via Ollama sur une machine grand
public** (ordre du téraoctet de VRAM/RAM). Ils s'utilisent via API et servent
ici uniquement de repère de qualité.
