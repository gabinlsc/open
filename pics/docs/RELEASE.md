# Publier une version

1. Mettre à jour la version (`pics/Cargo.toml`, `pics/crates/app/tauri.conf.json`, `pics/ui/package.json`).
2. Créer et pousser une étiquette : `git tag v0.1.0 && git push origin v0.1.0`.
3. Le workflow `release.yml` construit les installeurs NSIS et MSI et la version portable sur Windows, calcule les SHA-256, et crée une release GitHub avec les fichiers, `SHA256SUMS.txt` et `releases.json`.
4. Pour la page de téléchargement : copier `releases.json` dans `site/pics/` et déposer les fichiers dans `site/pics/download/` (la page lit `releases.json` et ne montre aucun lien tant qu'il est absent).

**État honnête** : ce workflow n'a pas pu être exécuté depuis l'environnement de développement (pas de runner Windows). Les binaires ne sont pas signés tant qu'un certificat n'est pas branché (voir l'étape « Signature » de `ci.yml`) : Windows SmartScreen affichera un avertissement.
