# Changelog

Toutes les évolutions notables de Cyonima-ia-code-agent sont consignées ici.
Le format suit [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) et le
versionnage [SemVer](https://semver.org/lang/fr/).

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
