from pydantic_settings import BaseSettings, SettingsConfigDict
from pydantic import Field


class Mem0Settings(BaseSettings):
    model_config = SettingsConfigDict(
        env_prefix="MEM0_",
        env_file=".env",
        env_file_encoding="utf-8",
        extra="ignore",
    )

    backend: str = Field(default="postgres", description="Mem0 backend type")
    backend_url: str = Field(
        default="postgres://postgres:postgres@localhost:5432/mem0",
        description="Backend connection URL",
    )
    collection_name: str = Field(
        default="pylos-memories", description="Mem0 collection/table name"
    )
    embedding_model: str = Field(
        default="nomic-embed-text-v2-moe-GGUF",
        description="Embedding model name",
    )
    embedding_provider: str = Field(
        default="ollama", description="Embedding provider (openai, ollama, ...)"
    )
    embedding_base_url: str = Field(
        default="http://192.168.0.58:11434/v1",
        description="Base URL for embedding API",
    )
    embedding_api_key: str | None = Field(
        default=None, description="API key for embedding provider"
    )
    max_context_tokens: int = Field(
        default=2048, ge=128, le=16384, description="Max tokens for context window"
    )
    ttl_seconds: int = Field(
        default=86400, ge=0, description="Default TTL for memory entries (0 = no TTL)"
    )
    search_limit: int = Field(
        default=10, ge=1, le=100, description="Default limit for search results"
    )
    sidecar_port: int = Field(
        default=7577, ge=1024, le=65535, description="Sidecar HTTP port"
    )
    sidecar_host: str = Field(
        default="0.0.0.0", description="Sidecar bind address"
    )
    graph_store_url: str | None = Field(
        default=None, description="Neo4j graph store connection URL"
    )
    graph_store_username: str | None = Field(
        default=None, description="Neo4j graph store username"
    )
    graph_store_password: str | None = Field(
        default=None, description="Neo4j graph store password"
    )
    log_level: str = Field(default="INFO", description="Logging level")

    @property
    def mem0_config(self) -> dict:
        config: dict[str, dict] = {
            "embedder": {
                "provider": self.embedding_provider,
                "config": {
                    "model": self.embedding_model,
                    "embedding_dims": 768,
                },
            },
        }
        if self.embedding_base_url:
            config["embedder"]["config"]["base_url"] = self.embedding_base_url
        if self.embedding_api_key:
            config["embedder"]["config"]["api_key"] = self.embedding_api_key

        if self.backend == "postgres":
            config["vector_store"] = {
                "provider": "postgres",
                "config": {
                    "connection_string": self.backend_url,
                    "collection_name": self.collection_name,
                },
            }
        elif self.backend == "qdrant":
            config["vector_store"] = {
                "provider": "qdrant",
                "config": {
                    "url": self.backend_url,
                    "collection_name": self.collection_name,
                },
            }
        elif self.backend == "chroma":
            config["vector_store"] = {
                "provider": "chroma",
                "config": {
                    "collection_name": self.collection_name,
                },
            }
        elif self.backend == "elasticsearch":
            config["vector_store"] = {
                "provider": "elasticsearch",
                "config": {
                    "connection_string": self.backend_url,
                    "collection_name": self.collection_name,
                },
            }

        config["version"] = "v1.0"
        if self.graph_store_url:
            config["graph_store"] = {
                "provider": "neo4j",
                "config": {
                    "url": self.graph_store_url,
                    "username": self.graph_store_username or "",
                    "password": self.graph_store_password or "",
                },
            }

        return config

    @classmethod
    def from_env(cls) -> "Mem0Settings":
        return cls()
