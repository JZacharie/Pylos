fn main() {
    // Créer le répertoire ui/dist s'il n'existe pas, nécessaire pour
    // rust-embed qui échoue à la compilation si le dossier est absent.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let dist = std::path::Path::new(&manifest_dir).join("../../ui/dist");
    std::fs::create_dir_all(&dist).ok();

    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../resources/pylos.ico");
        if let Err(e) = res.compile() {
            println!("cargo:warning=failed to embed icon resource: {}", e);
        }
    }
}
