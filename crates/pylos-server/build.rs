fn main() {
    // Créer le répertoire ui/dist s'il n'existe pas, nécessaire pour
    // rust-embed qui échoue à la compilation si le dossier est absent.
    let dist =
        std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../ui/dist");
    std::fs::create_dir_all(&dist).ok();

    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../resources/pylos.ico");
        res.compile().expect("failed to embed icon resource");
    }
}
