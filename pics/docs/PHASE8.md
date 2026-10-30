# Phase 8 — Productivité

## Livré
- **Palette de commandes (Ctrl+K)** : toutes les actions disponibles et tous les filtres, recherche approximative sans casse ni accents, navigation clavier, raccourcis affichés.
- **Récupération après plantage** : les documents modifiés sont recopiés toutes les 60 s en `.ocv` dans `recovery/` (dossier de configuration, ou `data/` en mode portable). La copie disparaît quand le document est enregistré ou fermé. Au lancement suivant, les copies restantes sont proposées (récupérer / abandonner) ; un document récupéré est « non enregistré » et demande un emplacement. Les noms de fichiers venus de l'interface sont validés (pas de sortie du dossier).
- **Traitement par lot** : applique un filtre (valeurs par défaut) à toutes les images d'un dossier, exporte en PNG/JPEG/WebP dans un autre dossier ; les originaux ne sont jamais écrasés ; une erreur sur un fichier n'arrête pas le lot et est rapportée.
- **Interface FR / EN** : menus, actions et outils sont traduits ; la langue se choisit dans Affichage (mémorisée localement, sinon langue du navigateur). Un test garantit que toute action du registre a une traduction.

## Non livré
- **Scripts (Lua/JS)**, **enregistrement d'actions** : non faits. Un moteur de script embarqué est une surface de sécurité (accès fichiers) qui demande sa propre conception ; je préfère ne rien livrer qu'un bac à sable approximatif.
- **Plans de travail multiples (artboards)**, **export multi-résolutions**, **comparaison avant/après**, **explorateur de fichiers** : non faits (le moteur n'a pas encore de redimensionnement ni de modèle de document à plusieurs cadres).
- **Traduction complète** : les boîtes de dialogue, panneaux et messages d'erreur restent en français ; seuls menus, actions et outils sont traduits. Les filtres et réglages gardent leurs libellés français venus du cœur.
- Le lot n'expose pas les paramètres du filtre (valeurs par défaut uniquement).
