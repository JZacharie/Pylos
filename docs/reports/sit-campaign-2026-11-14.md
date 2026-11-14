# Rapport de campagne SIT & Tests de contrat fournisseurs (2026-11-14)

**Issue** : `#132`  
**Date** : 2026-11-14  
**Périmètre** : Tests de contrat fournisseurs (`pylos-infrastructure`) et tests d'intégration système (SIT) de bout en bout (`pylos-server`).

## 1. Tests de contrat fournisseurs (`crates/pylos-infrastructure/tests/provider_contracts.rs`)

Fixtures versionnées dans `crates/pylos-infrastructure/tests/fixtures/` :
- `openai/chat_completion.json`
- `anthropic/messages_response.json`
- `gemini/generate_content_response.json`
- `bedrock/converse_response.json`
- `cohere/chat_response.json`

Commande exécutée :
```bash
cargo test -p pylos-infrastructure --test provider_contracts
```

Résultat observé :
- `test_openai_contract_fixture` : ok
- `test_anthropic_contract_fixture` : ok
- `test_gemini_contract_fixture` : ok
- `test_bedrock_contract_fixture` : ok
- `test_cohere_contract_fixture` : ok
- **Total** : `5 passed; 0 failed` (`0.00s`)

## 2. Campagne SIT de bout en bout (`crates/pylos-server/tests/sit_e2e.rs`)

Commande exécutée :
```bash
cargo test -p pylos-server --test sit_e2e
```

Résultat observé :
- `sit_multi_tenant_virtual_key_isolation` : ok (tenant autorisé sur `gpt-4o` -> `200 OK`, refusé sur `gpt-4o-mini` -> `502`/`403`)
- `sit_guardrails_block_and_allow` : ok (prompt contenant mot interdit -> `400 Bad Request`, prompt sain -> `200 OK`)
- `sit_budget_enforcement` : ok (clé virtuelle au budget dépassé -> `402 Payment Required`)
- `sit_provider_fallback_on_failure` : ok (bascule automatique du provider primaire en erreur 503 vers le provider secondaire -> `200 OK`)
- `sit_streaming_sse_end_to_end` : ok (flux SSE complet jusqu'à `data: [DONE]`)
- `sit_mcp_tools_list_and_ssrf_guard` : ok (`/mcp/tools/list` -> `200 OK`, blocage SSRF sur `http://127.0.0.1:8080/secret`)
- **Total** : `6 passed; 0 failed` (`0.76s`)
