# Changelog

Toutes les évolutions notables de Cyonima-ia-code-agent sont consignées ici.
Le format suit [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) et le
versionnage [SemVer](https://semver.org/lang/fr/).

## [1.1.0] — 2026-07-25

Agent autonome, panneau de raisonnement dédié et suppression de modèles.

### Ajouté
- **ThinkingPanel** : colonne centrale dédiée (25%) au flux de tokens « thinking »
  du modèle, avec auto-scroll et indicateur de streaming. Layout session
  passé de 2 à 3 colonnes (50% chat / 25% reasoning / 25% fichiers).
- **Suppression de modèles** : bouton « Désinstaller » dans le catalogue avec
  confirmation, appel `DELETE /api/delete` vers Ollama.
- **Agent autonome** : `write_file`, `edit_file`, `glob`, `grep` sont
  auto-approuvés (seul `bash` demande une approbation). System prompt réécrit
  pour encourager l'action autonome.
- **Workspace snapshot** : à chaque session, l'agent reçoit un aperçu du projet
  (arborescence + fichiers de config) injecté comme message system, pour
  comprendre le contexte sans outils supplémentaires.
- **`session:done` toujours émis** : corrige le bug où le frontend restait stuck
  en mode streaming après une erreur ou annulation.
- **`PermissionRequest` serde fix** : ajout de `rename_all = "camelCase"` sur la
  struct Rust, corrige le dialogue de permission qui n'apparaissait pas.
- **Permission logging** : chaque demande et réponse de permission est loguée
  (`tracing::info!`) pour faciliter le diagnostic.

### Corrigé
- Le dialogue de permission n'apparaissait pas car `request_id` sérialisé en
  snake_case côté Rust était lu en camelCase côté frontend (`undefined` → aucun
  match). Fix : `#[serde(rename_all = "camelCase")]` sur `PermissionRequest`.
- Le listener `permission:request` dans `PermissionDialog` était dans un composant
  pouvant ne pas être monté. Déplacé dans `App.tsx` (global) avec store zustand
  partagé `usePermissionsStore`.

### Notes
- `cargo fmt` requis avant chaque push (CI enforce `cargo fmt --all -- --check`).

[1.1.0]: https://github.com/LudovicBoudi/Cyonima-code-agent/releases/tag/v1.1.0

## [1.0.0] — 2026-07-20

Première release publique. Agent IA de code 100% local, gratuit et open source,
propulsé par [Ollama](https://ollama.com).

### Ajouté
- **Multi-session** parallèle avec persistance SQLite (`~/.cyonima/sessions.db`),
  restauration au démarrage, fork et suppression de sessions.
- **Inférence via Ollama** (HTTP local) avec streaming token par token et
  détection automatique des capacités du modèle (`tools`, `thinking`) via
  `/api/show`.
- **Modèles « thinking »** : affichage du raisonnement dans un bloc repliable et
  réglage de l'**intensité de raisonnement** (Auto / Désactivé / Faible / Moyen /
  Élevé) depuis la chatbox.
- **Indicateur d'usage de contexte** dans la chatbox (tokens du dernier tour vs
  taille de contexte du modèle).
- **Outils agent** sandboxés au workspace : `read_file`, `write_file`,
  `edit_file`, `glob`, `grep`, `bash`, avec **gateway de permissions** (auto /
  demande / refus) et prévisualisation avant exécution.
- **AGENTS.md** du workspace injecté comme instructions système (masqué de
  l'affichage, remplacé par un message de bienvenue).
- **Vue session en 2 colonnes** : conversation (75%) + **panneau des fichiers
  git** modifiés/ajoutés/supprimés/renommés du workspace (25%).
- **Catalogue de modèles** trié par RAM, séparé en « installés » / « disponibles »,
  avec garde-fou hardware (RAM/VRAM) et pull Ollama avec progression.
- **Sélection du modèle** dans la chatbox parmi les modèles Ollama installés.
- **Thème violet** (sombre) unique.
- Icône d'application, boutons Play/Stop, guide des modèles
  ([`docs/models-guide.md`](docs/models-guide.md)).
- **Packaging & release** : workflow GitHub Actions produisant les installateurs
  `.msi` (Windows), `.dmg` (macOS Intel + Apple Silicon), `.deb` et `.AppImage`
  (Linux), publiés en Release GitHub avec `SHA256SUMS.txt`.

### Notes
- Cette version se limite volontairement aux **capacités d'Ollama**. Les backends
  GGUF intégré (candle), les téléchargements GGUF directs, l'import de modèles
  custom, les providers d'API distantes et la recherche sémantique sont présents
  dans le code mais **désactivés** dans cette release (cf `docs/ROADMAP.md`).
- La signature de code (notarization macOS, signature Windows) s'active en
  fournissant les secrets correspondants au workflow de release ; sans eux, les
  installateurs sont produits non signés.

[1.0.0]: https://github.com/LudovicBoudi/Cyonima-code-agent/releases/tag/v1.0.0
