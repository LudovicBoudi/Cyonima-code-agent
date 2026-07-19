# AGENTS.md — Instructions par défaut pour les agents Cyonima IA

Ce fichier est lu par Cyonima-ia-code-agent quand il travaille dans ce dépôt et injecté dans le system prompt.
Vous pouvez créer un `AGENTS.md` à la racine de n'importe quel projet pour le personnaliser.

> **Cyonima** est une marque. Le produit est **Cyonima-ia-code-agent**.

## Style de code

- **Rust** : edition 2021, formatting `rustfmt.toml` (4 espaces), `clippy::all` sans warnings. Préférer les API `async`/`await` sur `tokio`. Imports groupés en `Std / External / Crate / Super`.
- **TypeScript** : `strict: true`, pas de `any` sans justification en commentaire, ESM, préférer `type` pour les alias simples et `interface` pour les objets extensibles.
- **Commits** : convention Conventional Commits (feat: / fix: / docs: / refactor: / chore: / test:), corps au présent, ligne < 72 caractères.

## Priorité au local

Cyonima-ia-code-agent est conçu pour tourner en local d'abord. Toute nouvelle fonctionnalité doit fonctionner sans réseau et sans compte distant. Les appels distants (API, model hub) sont optionnels et isolés derrière des flags/traits clairs.

## Taille des binaires < 50 Mo dans le repo

Ne jamais committer de fichier > 50 Mo dans le repo Git. Les LLM GGUF et les datasets lourds sont téléchargés à l'exécution ou hébergés via GitHub Releases. Le seul asset embarqué est l'embedder quantizé dans `src-tauri/resources/`.

## Architecture

- Tout backend IA implémente le trait `Provider` (voir `docs/ARCHITECTURE.md`). Ne pas introduire de logique spécifique à un backend en dehors de `providers/`.
- Les outils agent vivent dans `tools/` et passent TOUJOURS par le gateway `permissions/`. Pas de bypass.
- Les chemins de storage runtime sont lus depuis `config/` — ne jamais hardcoder `~/.cyonima/...` dans le code applicatif.

## Sécurité

- Les clés API ne doivent jamais être loggées, sérialisées en clair dans la config, ni écrites sur disque hors keyring OS.
- Les commandes `bash` exécutées par l'agent doivent être prévisualisées à l'utilisateur avant exécution.
- Le filesystem agent est sandboxé au workspace + dossiers explicitement autorisés.

## Préférences agents

- Réponses concises, pas de filler. Passez à l'action si la consigne est claire.
- Lorsqu'un test existe pour la zone modifiée, lancez-le et corrigez-le.
- Documentez les décisions non évidentes dans le code en commentaire bref, pas en prose longue.
- En cas de doute sur un choix d'architecture majeur, demandez plutôt que de partir sur une direction irreversible.