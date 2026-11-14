# Guardrails & Modération de Contenu — Pylos

## Modes de fonctionnement

Le plugin `guardrails` supporte deux modes via la configuration `mode` :

- **`monitor`** : détecte les violations (score de modération, mots-clés bloqués, tentatives de *prompt injection*), enrichit le contexte (`guardrail_triggered`, `guardrail_type`, `guardrail_detail`, `x-moderation-scores`) et journalise des alertes, **sans interrompre** la requête.
- **`block`** : refuse la requête dès la phase `pre_hook` en retournant `PylosError::GuardrailViolation`, traduit en réponse HTTP **`400 Bad Request`** avec le type d'erreur **`content_policy_violation`**.

## Normalisation anti-évasion

Avant l'évaluation des mots-clés et des règles d'injection, le texte est normalisé par `normalize_for_detection` :
- Suppression des caractères invisibles et zéro-largeur (`U+200B`, `U+200C`, `U+200D`, `U+FEFF`, `U+00AD`, `U+2060`, `U+200E`, `U+200F`).
- Repli des caractères *fullwidth* ASCII (`\u{FF01}..=\u{FF5E}`) et des homoglyphes cyrilliques courants (`а`, `е`, `о`, `р`, `с`, `х`, `і`, `у`, `к`, `м`, `т`).
- Passage en minuscules et fusion des espaces consécutifs.

## Limites connues des heuristiques

Les règles intégrées reposent sur des **expressions régulières et des listes lexicales statiques**, et non sur un classifieur sémantique :
1. **Faux positifs possibles** : certains termes techniques ou contextuels (sécurité informatique, administration système, fiction) peuvent augmenter le score si `moderation_threshold` est réglé trop bas.
2. **Contournements avancés** : les encodages arbitraires (Base64 décodé par le modèle, rot13, langues rares, paraphrases sémantiques) ou les attaques multi-tours ne sont pas interceptés par de simples regex.
3. **Couverture linguistique** : les catégories de modération sont principalement calibrées pour l'anglais et le français.

Pour des exigences de conformité strictes en production, il est recommandé de combiner ces guardrails rapides avec un classifieur dédié (ex. Llama Guard / NeMo Guardrails) en amont ou via un plugin externe.
