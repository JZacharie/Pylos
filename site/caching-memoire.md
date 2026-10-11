---
layout: default
title: Caching & Mémoire
description: Stratégies d'optimisation des jetons par cache TinyLFU, cache sémantique vectoriel Qdrant et graphe de mémoire Memgraph.
---

# Caching & Mémoire Cross-Agent

Pylos intègre des mécanismes avancés pour réduire drastiquement les coûts de tokens et doter les agents conversationnels d'une mémoire persistante structurée.

---

## ⚡ 1. Cache de Préfixe en Mémoire (TinyLFU)

Le cache de préfixe intercepte les requêtes strictement identiques (même modèle, même invite système et même historique de messages) directement dans la mémoire vive du serveur.

* **Technologie & Algorithme** : Moteur de cache `moka` en Rust, utilisant la politique d'éviction moderne **TinyLFU** (Tiny Least Frequently Used).
* **Gain financier & écologique** : **100% des tokens d'entrée et de sortie sont économisés** sur un *cache hit*.
* **Latence** : Réponse délivrée en **< 1 ms** (sans aucun appel réseau sortant).

```
Requête Client ──► Hash(Messages + Modèle) ──► Table TinyLFU (moka)
                                                     │
                                           Hit ──────┴─────► Réponse immédiate (<1ms)
```

---

## 🧠 2. Cache Sémantique Vectoriel (Qdrant)

Contrairement au cache de préfixe qui exige une égalité exacte caractère par caractère, le **cache sémantique** reconnaît des questions reformulées ayant le même sens.

### Fonctionnement :
1. Pylos génère le vecteur d'embedding de la requête utilisateur (ex. via un modèle d'embedding léger).
2. Pylos interroge la collection vectorielle hébergée sur **Qdrant** en calculant la **similitude cosinus**.
3. Si la similarité dépasse le seuil configuré (par exemple `0.92`), la réponse précédemment stockée est renvoyée immédiatement.

> [!TIP]
> **Exemple de correspondance sémantique :**
> * Requête initiale : *"Quelle est la politique de congés payés de l'entreprise ?"*
> * Seconde requête : *"Combien de jours de vacances ai-je par an ?"*
> Le cache sémantique identifie la similitude de sens et renvoie la réponse sans réinterroger le LLM payant.

---

## 🕸️ 3. Graphe de Mémoire Cross-Agent (Memgraph)

Pour permettre aux agents IA de mémoriser des faits complexes, des préférences utilisateurs ou des états de projets au fil de multiples sessions, Pylos intègre une connexion directe à **Memgraph** via le protocole **Bolt** (bibliothèque `neo4rs`) et le langage de requête **Cypher**.

### Les deux étapes du cycle de mémoire :

#### A. Le Pre-hook (Injection de Contexte)
Avant de transmettre la requête au modèle, Pylos extrait l'identifiant de l'utilisateur ou de l'entité et interroge le graphe Memgraph :

```cypher
MATCH (u:User {id: $user_id})-[r]->(fact:Fact)
RETURN u.name, type(r), fact.description
LIMIT 10
```

Ces relations sont injectées automatiquement sous forme de contexte structuré dans l'invite système du modèle.

#### B. Le Post-hook (Extraction & Fusion Automatique)
Le modèle de langage peut émettre des balises de mémorisation dans sa réponse :

```xml
<memory>Jean-Dupont|PREFERE|Reponses_Concis</memory>
<memory>Projet-Alpha|UTILISE_STACK|Rust_Axum</memory>
```

Le post-hook Pylos intercepte ces balises, les supprime du flux final visible par l'utilisateur, et exécute immédiatement les requêtes de fusion dans Memgraph :

```cypher
MERGE (a:Entity {name: 'Jean-Dupont'})
MERGE (b:Entity {name: 'Reponses_Concis'})
MERGE (a)-[:PREFERE]->(b)
```

---

## 📚 4. RAG Unifié (Génération Augmentée par Récupération)

Pour des modèles configurés comme `graphon-rag-emails` ou `mnemosyne-search`, Pylos agit comme un orchestrateur RAG automatique :

1. Récupération des k documents les plus pertinents depuis la collection vectorielle Qdrant.
2. Synthèse et injection transparente des extraits documentaires en tête de prompt.
3. Transmission de la requête enrichie au LLM de votre choix (OpenAI, Claude, etc.).
