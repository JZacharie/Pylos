# Runbook d'Exploitation Kubernetes — Pylos

Ce runbook décrit les procédures opérationnelles pour exploiter Pylos en production sur Kubernetes via le chart Helm `helm/pylos`.

## 1. Prérequis Multi-Répliques (PostgreSQL obligatoire)

Pylos utilise SQLite par défaut sur un PVC `ReadWriteOnce`. Le déploiement multi-répliques (`backend.replicaCount > 1` ou `backend.autoscaling.enabled=true`) est **bloqué par le chart Helm** tant que `database.enabled=true` (PostgreSQL) n'est pas configuré :

```yaml
database:
  enabled: true
  existingSecret: "pylos-db-secret"
  existingSecretKey: "DATABASE_URL"

backend:
  replicaCount: 2
  podDisruptionBudget:
    enabled: true
    minAvailable: 1
```

## 2. Drain de Nœud & Maintenance Planifiée

1. Vérifier que le `PodDisruptionBudget` est actif si `replicaCount > 1` :
   ```bash
   kubectl get pdb -n pylos
   ```
2. Drainer le nœud cible en respectant les budgets d'interruption :
   ```bash
   kubectl drain <node-name> --ignore-daemonsets --delete-emptydir-data
   ```
3. Vérifier que les nouveaux pods passent `readinessProbe` (`/health`) :
   ```bash
   kubectl rollout status deployment/pylos -n pylos
   ```
4. Réactiver l'ordonnancement après maintenance :
   ```bash
   kubectl uncordon <node-name>
   ```

## 3. Rollback Helm

1. Consulter l'historique des révisions :
   ```bash
   helm history pylos -n pylos
   ```
2. Revenir à la révision stable précédente :
   ```bash
   helm rollback pylos <REVISION> -n pylos --wait
   ```
3. Vérifier l'état des pods et des logs :
   ```bash
   kubectl get pods -n pylos
   kubectl logs -l app=pylos -n pylos --tail=100
   ```

## 4. Rotation des Clés & Secrets

### Clés API Providers & Clé Admin (`OPENCODE_API_KEY`)
1. Mettre à jour le secret source dans votre gestionnaire de secrets (Vault / ExternalSecrets) ou directement dans le secret Kubernetes `pylos-secret`.
2. Si ExternalSecrets est activé (`secretStore.enabled=true`), forcer la synchronisation :
   ```bash
   kubectl annotate externalsecret -n pylos --all force-sync=$(date +%s) --overwrite
   ```
3. Redémarrer proprement le déploiement pour charger les nouvelles variables d'environnement :
   ```bash
   kubectl rollout restart deployment/pylos -n pylos
   kubectl rollout status deployment/pylos -n pylos
   ```

### Virtual Keys Pylos
1. Créer une nouvelle Virtual Key via l'API d'administration (`POST /admin/virtual-keys`) ou l'interface UI (`Virtual Keys`).
2. Migrer les clients vers la nouvelle clé.
3. Révoquer l'ancienne Virtual Key (`DELETE /admin/virtual-keys/<id>`).
