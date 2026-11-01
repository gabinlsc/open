# Déployer open.glsc.fr

Tout se lance **sur le serveur** (Linux, nginx, accès root) ; le site est un dossier de fichiers statiques.

```bash
# Première installation : site + config nginx + dernière version publiée
curl -fsSL https://raw.githubusercontent.com/gabinlsc/openpics/main/deploy/install.sh | sudo bash -s -- --nginx --release

# Mise à jour (même commande, sans --nginx)
curl -fsSL https://raw.githubusercontent.com/gabinlsc/openpics/main/deploy/install.sh | sudo bash -s -- --release
```

Avant `--nginx` : DNS de `open.glsc.fr` vers le serveur, puis `sudo certbot certonly --nginx -d open.glsc.fr` (la config attend le certificat Let's Encrypt).
Pour relire le script avant de l'exécuter : `curl -fsSL …/install.sh -o install.sh && less install.sh && sudo bash install.sh --release`.

Options : `--dir` (défaut `/var/www/open.glsc.fr`), `--ref` (branche ou étiquette), `--release`, `--nginx`, `--dry-run`.

`--release` lit `releases.json` de la dernière release GitHub (créée par le workflow `release.yml` quand on pousse une étiquette `vX.Y.Z`), télécharge chaque fichier listé et **refuse tout fichier dont le SHA-256 ne correspond pas**. Tant qu'aucune release n'existe, la page de téléchargement affiche « Bientôt ».
Les téléchargements déjà publiés (`pics/download/`, `pics/releases.json`) ne sont jamais supprimés par une mise à jour du site.
