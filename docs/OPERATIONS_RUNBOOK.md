# Guide d'Exploitation Pylos (Runbook)

## 1. Vérification Santé & Observabilité
- **Health check** : `GET /health`
- **Métriques Prometheus** : `GET /metrics` (`pylos_inference_requests_total`, `pylos_tenant_requests_total`, `pylos_inference_cost_usd`, `pylos_admin_audit_events_total`)
- **Disponibilité & FinOps** : `GET /v1/providers`, `GET /v1/routing/finops`, `GET /v1/quotas`

## 2. Rotation des Secrets
1. **`PYLOS_ADMIN_KEY`** : Mettre à jour la variable d'environnement du déploiement et redémarrer / appeler `POST /config/reload`.
2. **`PYLOS_JWT_SECRET`** : Faire tourner la clé (invalide les sessions JWT en cours ; les utilisateurs OAuth se reconnectent).
3. **`SECRET_ENCRYPTION_KEY`** : Clé AES-256-GCM protégeant les clés providers chiffrées en base (`ENC[...]`). Sauvegarder l'ancienne clé avant toute rotation.
4. **Comptes Antigravity (`AGY_ACCOUNT`)** : Mettre à jour les comptes dans `pylos.json` / secrets puis recharger via `POST /config/reload`.

## 3. Sauvegarde & Restauration
- **SQLite** : Sauvegarder le fichier `pylos.db` (ou volume PVC `/data/pylos.db`) à chaud via `.backup` ou snapshot PVC.
- **PostgreSQL** : `pg_dump -Fc` sur la base cible ; le schéma est géré automatiquement par les migrations au démarrage.
- **Configuration** : Versionner `pylos.json` sans secrets en clair (utiliser `${ENV_VAR}` ou `ENC[...]`).
