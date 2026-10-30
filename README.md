# Open

Suite créative libre et locale pour Windows.

- `pics/` — OpenPics, éditeur d'images (architecture : [`pics/docs/ARCHITECTURE.md`](pics/docs/ARCHITECTURE.md))
- `site/` — site statique `open.glsc.fr` (accueil + pages de téléchargement par module)
- `deploy/` — scripts de déploiement (rsync/SSH)

Déploiement : `DEPLOY_HOST=user@serveur DEPLOY_PATH=/var/www/open.glsc.fr deploy/deploy-site.sh`
