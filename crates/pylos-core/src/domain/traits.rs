use crate::{
    domain::{
        embedding::{EmbeddingRequest, EmbeddingResponse},
        provider::ProviderConfig,
        request::{PylosRequest, PylosResponse, StreamChunk},
    },
    error::PylosError,
};
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

/// Type alias pour un stream de chunks SSE
pub type ChunkStream = Pin<Box<dyn Stream<Item = Result<StreamChunk, PylosError>> + Send>>;

/// Trait central — chaque provider implémente cette interface
#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;

    /// Envoie une requête non-streaming et retourne la réponse complète
    /// Les providers doivent gérer PylosRequest::ChatCompletion.
    /// PylosRequest::TextCompletion est converti en ChatCompletion par l'orchestrateur.
    /// PylosRequest::Embedding doit retourner Err(Unsupported) ici.
    async fn complete(
        &self,
        request: &PylosRequest,
        config: &ProviderConfig,
    ) -> Result<PylosResponse, PylosError>;

    /// Envoie une requête streaming
    async fn stream(
        &self,
        request: &PylosRequest,
        config: &ProviderConfig,
    ) -> Result<ChunkStream, PylosError>;

    /// Calcule des embeddings
    async fn embed(
        &self,
        _request: &EmbeddingRequest,
        _config: &ProviderConfig,
    ) -> Result<EmbeddingResponse, PylosError> {
        Err(PylosError::Unsupported(format!(
            "Provider '{}' does not support embeddings",
            self.name()
        )))
    }

    /// Génère des images
    async fn generate_image(
        &self,
        _request: &crate::domain::image::ImageRequest,
        _config: &ProviderConfig,
    ) -> Result<crate::domain::image::ImageResponse, PylosError> {
        Err(PylosError::Unsupported(format!(
            "Provider '{}' does not support image generation",
            self.name()
        )))
    }

    /// Vérifie la santé du provider
    async fn health_check(&self, config: &ProviderConfig) -> Result<(), PylosError> {
        let _ = config;
        Ok(())
    }
}

/// Trait pour les plugins pre/post hook
#[async_trait]
pub trait LlmPlugin: Send + Sync {
    fn name(&self) -> &str;

    /// Priorité d'exécution dans le pipeline (plus petit = exécuté plus tôt en `pre_hook`,
    /// et plus tard en `post_hook` / `post_stream_chunk`).
    /// Ordre recommandé :
    /// - `-100..=-10` : Gouvernance, Rate-Limit (-80), Budget (-70), Sécurité/Guardrails (-50)
    /// - `0..=49`    : Caches & Routage (défaut 0)
    /// - `50..=99`   : Transformations / Structured Output (50)
    /// - `100..`     : Observabilité / OTel (100)
    fn priority(&self) -> i32 {
        0
    }

    async fn pre_hook(
        &self,
        request: &mut PylosRequest,
        ctx: &mut crate::domain::request::RequestContext,
    ) -> Result<Option<PylosResponse>, PylosError> {
        let _ = (request, ctx);
        Ok(None)
    }

    async fn post_hook(
        &self,
        request: &PylosRequest,
        response: &mut PylosResponse,
        ctx: &mut crate::domain::request::RequestContext,
    ) -> Result<(), PylosError> {
        let _ = (request, response, ctx);
        Ok(())
    }

    /// Post-traitement synchrone appliqué à chaque chunk de streaming avant envoi au client.
    fn post_stream_chunk(
        &self,
        chunk: &mut crate::domain::request::StreamChunk,
        ctx: &mut crate::domain::request::RequestContext,
    ) -> Result<(), PylosError> {
        let _ = (chunk, ctx);
        Ok(())
    }
}
