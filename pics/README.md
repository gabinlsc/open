# OpenPics

Éditeur d'images raster libre et local pour Windows 10/11. Licence **GPL-3.0-only** (voir `../LICENSE`).
Aucun compte, aucune télémétrie : l'application ne fait aucune requête réseau.

- Architecture : [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- État de la phase en cours : [`docs/PHASE1.md`](docs/PHASE1.md)

## Prérequis

- Rust stable (voir `rust-toolchain.toml`) et Node.js 22
- Windows : WebView2 (inclus dans Windows 11, installé avec Windows 10 récent)
- Linux/macOS : seuls les crates `core`, `ops`, `io`, `gpu` et la logique de l'application se construisent ; la coque Tauri et le canvas natif sont propres à Windows

## Commandes

```bash
# Tests de tout ce qui ne dépend pas de la coque (toutes plateformes)
cargo test --workspace --exclude openpics-app
cargo test -p openpics-app --no-default-features

# Lint et format
cargo fmt --all --check
cargo clippy --workspace --exclude openpics-app --all-targets -- -D warnings

# Benchmarks (criterion)
cargo bench -p openpics-core -p openpics-ops -p openpics-io

# Audit des licences (cargo install --locked cargo-deny)
cargo deny check licenses bans sources

# Interface seule
cd ui && npm ci && npm run typecheck && npm test && npm run build
```

Sous Windows, application complète :

```bash
cd ui && npm ci
cd ../crates/app
../../ui/node_modules/.bin/tauri dev      # développement
../../ui/node_modules/.bin/tauri build    # installeurs NSIS et MSI dans target/release/bundle/
```

Les tests de rendu GPU utilisent la carte graphique de la machine ; sans adaptateur ils s'ignorent avec un message, sauf si `OPENPICS_REQUIRE_GPU=1` est défini (c'est le cas en CI, avec un rasteriseur Vulkan logiciel).

## Version portable

Un fichier vide nommé `portable` placé à côté de `OpenPics.exe` range les préférences dans `data\` à côté de l'exécutable ; rien n'est écrit ailleurs.

## Types partagés avec l'interface

Les types TypeScript de `ui/src/gen/` sont générés depuis Rust (ts-rs) par `cargo test -p openpics-app --no-default-features` ; ne les modifiez pas à la main (la CI vérifie qu'ils sont à jour).
