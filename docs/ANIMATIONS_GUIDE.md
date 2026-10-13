# 🎬 Guide des Animations Techniques en JavaScript (Zero-Asset)

Ce document décrit la méthode, les outils et les compétences (**skills**) intégrés dans le dépôt **Pylos** pour concevoir des animations didactiques sans aucun fichier image externe, selon la technique popularisée par **Claude Opus 5.5** et détaillée dans le blog [iArt AI — JavaScript Animation](https://www.iart.ai/blog/ai-javascript-animation).

---

## 💡 Principes Fondamentaux

Contrairement aux vidéos IA générées par modèles de diffusion ou aux captures d'écran statiques :
1. **Chaque pixel est calculé par du code** : Un script JavaScript unique peint l'intégralité de l'image sur un `<canvas>` HTML5 à l'instant `t`.
2. **Zéro asset externe** : Aucun fichier PNG, SVG, police externe ou vidéo n'est chargé. La page HTML pèse moins de 30 Ko et s'exécute directement dans n'importe quel navigateur.
3. **Fonction pure du temps `draw(frame)`** : Pas d'état accumulé ni d'imprécision temporelle. Le moteur headless (Chromium via Playwright) peut sauter précisément à n'importe quelle image (frame 0, 150, 300...) pour capturer un rendu MP4 déterministe.
4. **Aléatoire avec graine fixe (`seeded PRNG`)** : Les textures, légers frémissements (*line boil*) et particules utilisent un générateur pseudo-aléatoire déterministe (`hash2` / `rng(seed)`), garantissant zéro sautillement d'une frame à l'autre.
5. **Bande-son procédurale synchronisée (Web Audio API)** : La musique, les pulsations et les effets sonores d'événements sont synthétisés directement en code via `window.SCORE(ac)`. Les transitions visuelles (*cuts*) tombent exactement sur les temps forts (*beats*).

---

## 🛠️ Skills & Outils Installés

Les compétences d'agents sont versionnées dans le dossier [`.agents/skills/`](file:///e:/git/JZacharie/Pylos/.agents/skills) et enregistrées dans `skills-lock.json` :

| Skill | Emplacement | Rôle |
|---|---|---|
| **`javascript-animation`** | [`.agents/skills/javascript-animation/`](file:///e:/git/JZacharie/Pylos/.agents/skills/javascript-animation) | Bibliothèque de dessin canvas, gestion du storyboard, export headless MP4 et audits de conformité |
| **`soundtrack`** | [`.agents/skills/soundtrack/`](file:///e:/git/JZacharie/Pylos/.agents/skills/soundtrack) | Synthèse audio procédurale Web Audio API, tempo map et vérification de synchronisation |

### Scripts Utilitaires Disponibles

* **`render.mjs`** : Lance Chromium en tâche de fond, capture frame par frame le canvas de la page, génère le flux audio avec `OfflineAudioContext`, et produit une vidéo MP4 H.264 ou une planche de contact (*contact sheet*).
* **`asset-audit.mjs`** : Analyse le code HTML et surveille les requêtes réseau pour certifier qu'aucun fichier distant n'est importé (garantie *Zero-Asset*).
* **`layout-check.mjs`** : Détecte les chevauchements accidentels de textes ou d'éléments visuels.

---

## 🚀 Commandes Rapides (npm)

Dans la racine du projet Pylos :

```bash
# 1. Auditer la conformité zero-asset de l'animation
npm run anim:audit

# 2. Exporter la vidéo MP4 complète avec bande-son synthétisée (H.264)
npm run anim:render

# 3. Générer un GIF animé optimisé (Lanczos 12fps) pour GitHub / Markdown
npm run anim:gif

# 4. Générer des captures instantanées (stills) aux instants clés
npm run anim:stills
```

---

## 📐 Architecture de l'Animation Pylos

L'animation de référence du cycle de vie des requêtes Pylos se trouve dans :
* **Source HTML/Canvas interactif** : [`animations/pylos-architecture/pylos-architecture.html`](file:///e:/git/JZacharie/Pylos/animations/pylos-architecture/pylos-architecture.html)
* **Copie intégrée au site de documentation** : [`site/assets/animations/pylos-architecture.html`](file:///e:/git/JZacharie/Pylos/site/assets/animations/pylos-architecture.html)
* **Vidéo MP4 de référence** : [`docs/pylos-architecture.mp4`](file:///e:/git/JZacharie/Pylos/docs/pylos-architecture.mp4)
* **GIF animé pour la documentation** : [`docs/pylos-architecture.gif`](file:///e:/git/JZacharie/Pylos/docs/pylos-architecture.gif)

### Structure Narrative (4 Actes - 18 secondes)

```
0.0s ──────── 4.5s ──────── 9.0s ──────── 13.5s ──────── 18.0s
[ Acte 1 ]       [ Acte 2 ]       [ Acte 3 ]       [ Acte 4 ]
 Ingress          Cache L1         Cache Miss       Streaming SSE
 Axum Rust        TinyLFU HIT      Routage LLM      Graph Memgraph
 <2ms overhead    0-token retour   Claude / Bedrock Audit Prometheus
```

1. **Acte 1 (0.0s - 4.5s)** : La requête du client arrive (format compatible OpenAI). Le middleware Axum en Rust applique l'authentification de la clé virtuelle (`sk-pylos-*`), le limiteur Token Bucket et le contrôle budgétaire USD.
2. **Acte 2 (4.5s - 9.0s)** : Les pré-plugins masquent les données sensibles (PII). Le cache mémoire TinyLFU identifie un **Cache Hit** : la réponse est renvoyée instantanément (< 0.2ms) avec **0 jeton facturé**.
3. **Acte 3 (9.0s - 13.5s)** : Sur une requête inédite (**Cache Miss**), le routeur dynamique pondéré évalue la santé et la latence des fournisseurs et achemine la charge vers l'adaptateur optimal (ex. Claude 3.5 Sonnet).
4. **Acte 4 (13.5s - 18.0s)** : Les jetons sont renvoyés en flux continu (SSE), le graphe sémantique Memgraph est mis à jour en arrière-plan et les compteurs Prometheus s'incrémentent.

---

## 🔮 Comment Créer de Nouvelles Animations

Pour ajouter une nouvelle animation (par exemple un focus sur le proxy MCP, le basculement dynamique en cas de panne *circuit breaker*, ou le cache sémantique vectoriel Qdrant) :

1. **Copier le gabarit** :
   Créer un dossier `animations/<nom-du-sujet>/` et dupliquer `animations/pylos-architecture/pylos-architecture.html`.
2. **Respecter le contrat d'interface** :
   ```javascript
   window.W = 1920;
   window.H = 1080;
   window.FPS = 30;
   window.FRAMES = FPS * DUR;
   window.CUES = [0, 4.5, 9.0, ...]; // Instants clés en secondes
   window.draw = function(frame) {
     const t = (frame % FRAMES) / FPS;
     // Dessiner TOUT à partir de t sans état persistant
   };
   window.SCORE = function(ac) {
     // Web Audio API : synthétiseur, pads, clics rythmiques
   };
   ```
3. **Tester dans le navigateur** :
   Ouvrir directement le fichier HTML dans un navigateur pour bénéficier du lecteur interactif (barre de scrubbing, lecture/pause, activation audio).
4. **Exécuter l'audit et le rendu** :
   ```bash
   node .agents/skills/javascript-animation/scripts/asset-audit.mjs animations/<nom>/<fichier>.html
   node .agents/skills/javascript-animation/scripts/render.mjs animations/<nom>/<fichier>.html docs/<nom>.mp4
   ```
5. **Intégrer le GIF et la vidéo dans la documentation Markdown ou le site Jekyll**.
