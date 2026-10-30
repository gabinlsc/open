fn main() {
    // Sans la coque, il n'y a ni fenêtre ni ressources Windows à préparer.
    if std::env::var_os("CARGO_FEATURE_SHELL").is_some() {
        tauri_build::build();
    }
}
