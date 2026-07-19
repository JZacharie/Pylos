pub mod cache_aligner;
pub mod caveman;
pub mod smart_crusher;

pub use caveman::CavemanMode;
use pylos_core::domain::openai::ChatCompletionRequest;

/// Applique les différentes passes d'optimisation / compression sur la requête LLM
/// Retourne le nombre d'octets économisés
pub fn optimize_request(
    request: &mut ChatCompletionRequest,
    caveman_mode: CavemanMode,
    shrink_input: bool,
) -> usize {
    // 1. Minifier le JSON (Smart Crusher)
    let mut saved_bytes = smart_crusher::minify_json_content(request);

    // 2. Appliquer Caveman (Input Shrinking et Injection Prompt)
    saved_bytes += caveman::apply_caveman(request, caveman_mode, shrink_input);

    // 3. Réaligner le cache (Cache Aligner)
    cache_aligner::align_system_messages(request);

    saved_bytes
}

/// Serialise les messages de la requête en JSON pour le logging du payload compressé.
pub fn caveman_input_json(request: &ChatCompletionRequest) -> Option<String> {
    serde_json::to_string(&request.messages).ok()
}
