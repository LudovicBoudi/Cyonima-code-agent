# Dossier des ressources < 50 Mo embarquées par Tauri.
#
# Ce dossier contient l'embedder local `all-MiniLM-L6-v2` quantizé (~23 Mo)
# pour la recherche sémantique zero-config (jalon J8). Il NE doit JAMAIS
# contenir de fichier > 50 Mo : les LLM sont téléchargés via le catalogue
# `docs/models-catalog.toml` (cf `docs/ARCHITECTURE.md`).
#
# Placez ici l'embedder GGUF avant release :
#   all-MiniLM-L6-v2-q8_0.gguf