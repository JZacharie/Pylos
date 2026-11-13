import os
import sys
from pptx import Presentation
from pptx.util import Inches, Pt
from pptx.dml.color import RGBColor
from pptx.enum.text import PP_ALIGN, MSO_ANCHOR
from pptx.enum.shapes import MSO_SHAPE

def build_presentation(output_path):
    prs = Presentation()
    prs.slide_width = Inches(13.333)
    prs.slide_height = Inches(7.5)
    blank_layout = prs.slide_layouts[6]  # completely blank

    # Color Palette from Template (docs/6406.eps / 6406.jpg)
    COLOR_GREEN     = RGBColor(30, 77, 56)    # #1E4D38 Forest Green
    COLOR_BURGUNDY  = RGBColor(125, 29, 45)   # #7D1D2D Crimson / Wine
    COLOR_NAVY      = RGBColor(30, 43, 55)    # #1E2B37 Dark Slate / Navy
    COLOR_ORANGE    = RGBColor(229, 107, 45)  # #E56B2D Warm Terracotta / Orange
    
    COLOR_BG_LIGHT  = RGBColor(246, 248, 250) # #F6F8FA Light grayish white
    COLOR_CARD_BG   = RGBColor(255, 255, 255) # Pure White
    COLOR_CARD_MUTED= RGBColor(241, 243, 246) # Sub-card light gray
    COLOR_BORDER    = RGBColor(222, 226, 230) # Border gray
    COLOR_TEXT_DARK = RGBColor(33, 37, 41)    # #212529 Dark Charcoal
    COLOR_TEXT_MUTED= RGBColor(108, 117, 125) # #6C757D Gray text
    COLOR_TEXT_LIGHT= RGBColor(255, 255, 255) # White
    COLOR_CODE_BG   = RGBColor(28, 33, 40)    # GitHub dark code bg
    COLOR_CODE_TEXT = RGBColor(230, 237, 243) # Light code text

    FONT_TITLE = "Segoe UI"
    FONT_BODY  = "Segoe UI"
    FONT_CODE  = "Consolas"

    def apply_header(slide, title_text, category_text="PROCESS PAGE", slide_num=1):
        # Background fill
        bg = slide.shapes.add_shape(MSO_SHAPE.RECTANGLE, 0, 0, Inches(13.333), Inches(7.5))
        bg.fill.solid()
        bg.fill.fore_color.rgb = COLOR_BG_LIGHT
        bg.line.fill.background()

        # Category pill/label top left
        cat_box = slide.shapes.add_textbox(Inches(0.8), Inches(0.4), Inches(4.0), Inches(0.4))
        tf_cat = cat_box.text_frame
        tf_cat.word_wrap = True
        tf_cat.margin_left = tf_cat.margin_top = tf_cat.margin_right = tf_cat.margin_bottom = 0
        p_cat = tf_cat.paragraphs[0]
        p_cat.text = category_text.upper()
        p_cat.font.name = FONT_TITLE
        p_cat.font.size = Pt(11)
        p_cat.font.bold = True
        p_cat.font.color.rgb = COLOR_TEXT_MUTED

        # Thin rule under category
        rule = slide.shapes.add_shape(MSO_SHAPE.RECTANGLE, Inches(2.3), Inches(0.55), Inches(8.5), Inches(0.015))
        rule.fill.solid()
        rule.fill.fore_color.rgb = COLOR_BORDER
        rule.line.fill.background()

        # Logotype top right
        logo_box = slide.shapes.add_textbox(Inches(10.5), Inches(0.35), Inches(2.0), Inches(0.4))
        tf_logo = logo_box.text_frame
        tf_logo.word_wrap = True
        tf_logo.margin_left = tf_logo.margin_top = tf_logo.margin_right = tf_logo.margin_bottom = 0
        p_logo = tf_logo.paragraphs[0]
        p_logo.alignment = PP_ALIGN.RIGHT
        p_logo.text = "PYLOS AI GATEWAY"
        p_logo.font.name = FONT_TITLE
        p_logo.font.size = Pt(11)
        p_logo.font.bold = True
        p_logo.font.color.rgb = COLOR_NAVY

        # Main slide title
        title_box = slide.shapes.add_textbox(Inches(0.8), Inches(0.8), Inches(11.7), Inches(0.6))
        tf_title = title_box.text_frame
        tf_title.word_wrap = True
        tf_title.margin_left = tf_title.margin_top = tf_title.margin_right = tf_title.margin_bottom = 0
        p_t = tf_title.paragraphs[0]
        p_t.text = title_text
        p_t.font.name = FONT_TITLE
        p_t.font.size = Pt(22)
        p_t.font.bold = True
        p_t.font.color.rgb = COLOR_TEXT_DARK

        # Page number pill bottom right
        page_box = slide.shapes.add_shape(MSO_SHAPE.ROUNDED_RECTANGLE, Inches(12.1), Inches(6.9), Inches(0.55), Inches(0.3))
        page_box.fill.solid()
        page_box.fill.fore_color.rgb = COLOR_BORDER
        page_box.line.fill.background()
        tf_page = page_box.text_frame
        tf_page.vertical_anchor = MSO_ANCHOR.MIDDLE
        p_p = tf_page.paragraphs[0]
        p_p.alignment = PP_ALIGN.CENTER
        p_p.text = f"{slide_num:02d}"
        p_p.font.name = FONT_TITLE
        p_p.font.size = Pt(10)
        p_p.font.bold = True
        p_p.font.color.rgb = COLOR_NAVY

    def add_card(slide, left, top, width, height, bg_color=COLOR_CARD_BG, border_color=COLOR_BORDER):
        card = slide.shapes.add_shape(MSO_SHAPE.ROUNDED_RECTANGLE, left, top, width, height)
        card.fill.solid()
        card.fill.fore_color.rgb = bg_color
        if border_color:
            card.line.color.rgb = border_color
            card.line.width = Pt(1)
        else:
            card.line.fill.background()
        return card

    def add_badge(slide, left, top, width, height, text, bg_color, text_color=COLOR_TEXT_LIGHT, font_size=10):
        badge = slide.shapes.add_shape(MSO_SHAPE.ROUNDED_RECTANGLE, left, top, width, height)
        badge.fill.solid()
        badge.fill.fore_color.rgb = bg_color
        badge.line.fill.background()
        tf = badge.text_frame
        tf.vertical_anchor = MSO_ANCHOR.MIDDLE
        tf.margin_left = tf.margin_top = tf.margin_right = tf.margin_bottom = 0
        p = tf.paragraphs[0]
        p.alignment = PP_ALIGN.CENTER
        p.text = text
        p.font.name = FONT_TITLE
        p.font.size = Pt(font_size)
        p.font.bold = True
        p.font.color.rgb = text_color
        return badge

    # =========================================================================
    # SLIDE 1: Title Slide / Couverture
    # =========================================================================
    s1 = prs.slides.add_slide(blank_layout)
    # Background full
    bg1 = s1.shapes.add_shape(MSO_SHAPE.RECTANGLE, 0, 0, Inches(13.333), Inches(7.5))
    bg1.fill.solid()
    bg1.fill.fore_color.rgb = COLOR_NAVY
    bg1.line.fill.background()

    # White inner card
    card1 = add_card(s1, Inches(0.8), Inches(0.8), Inches(11.733), Inches(5.9), bg_color=COLOR_CARD_BG, border_color=None)

    # Accent top band
    band1 = s1.shapes.add_shape(MSO_SHAPE.RECTANGLE, Inches(0.8), Inches(0.8), Inches(11.733), Inches(0.15))
    band1.fill.solid()
    band1.fill.fore_color.rgb = COLOR_ORANGE
    band1.line.fill.background()

    # Logo / Tagline
    tag1 = s1.shapes.add_textbox(Inches(1.5), Inches(1.5), Inches(10.0), Inches(0.4))
    tf_tag1 = tag1.text_frame
    p = tf_tag1.paragraphs[0]
    p.text = "ARCHITECTURE & DOSSIER TECHNIQUE D'INGÉNIERIE"
    p.font.name = FONT_TITLE
    p.font.size = Pt(13)
    p.font.bold = True
    p.font.color.rgb = COLOR_BURGUNDY

    # Big Title
    title1 = s1.shapes.add_textbox(Inches(1.5), Inches(1.9), Inches(10.3), Inches(1.4))
    tf_t1 = title1.text_frame
    tf_t1.word_wrap = True
    p = tf_t1.paragraphs[0]
    p.text = "Pylos — Passerelle LLM & Proxy MCP"
    p.font.name = FONT_TITLE
    p.font.size = Pt(36)
    p.font.bold = True
    p.font.color.rgb = COLOR_NAVY

    p2 = tf_t1.add_paragraph()
    p2.text = "Architecture Hexagonale Rust, Gouvernance FinOps, Cache Avancé & RAG Graph"
    p2.font.name = FONT_TITLE
    p2.font.size = Pt(18)
    p2.font.color.rgb = COLOR_TEXT_MUTED

    # 4 Thematic Pill Badges (Template inspired: Green, Burgundy, Navy, Orange)
    pills_data = [
        ("⚡ LATENCE < 2ms", "Moteur async Rust & Tokio", COLOR_GREEN),
        ("💰 FINOPS & BUDGETS", "Clés virtuelles & quotas $", COLOR_BURGUNDY),
        ("🛡️ RÉSILIENCE ACTIVE", "Multi-providers & Fallback", COLOR_NAVY),
        ("🧠 RAG & MÉMOIRE", "Qdrant + Graphe Memgraph", COLOR_ORANGE)
    ]
    pill_w = Inches(2.45)
    pill_h = Inches(1.3)
    for i, (badge_title, badge_desc, color) in enumerate(pills_data):
        bx = Inches(1.5) + i * Inches(2.6)
        by = Inches(3.6)
        c = add_card(s1, bx, by, pill_w, pill_h, bg_color=COLOR_BG_LIGHT, border_color=color)
        
        # Color bar on top of card
        top_bar = s1.shapes.add_shape(MSO_SHAPE.RECTANGLE, bx, by, pill_w, Inches(0.08))
        top_bar.fill.solid()
        top_bar.fill.fore_color.rgb = color
        top_bar.line.fill.background()

        tb = s1.shapes.add_textbox(bx + Inches(0.15), by + Inches(0.18), pill_w - Inches(0.3), pill_h - Inches(0.3))
        tf = tb.text_frame
        tf.word_wrap = True
        p_bt = tf.paragraphs[0]
        p_bt.text = badge_title
        p_bt.font.name = FONT_TITLE
        p_bt.font.size = Pt(11)
        p_bt.font.bold = True
        p_bt.font.color.rgb = color

        p_bd = tf.add_paragraph()
        p_bd.text = badge_desc
        p_bd.font.name = FONT_BODY
        p_bd.font.size = Pt(9.5)
        p_bd.font.color.rgb = COLOR_TEXT_DARK

    # Footer metadata box
    foot = s1.shapes.add_textbox(Inches(1.5), Inches(5.4), Inches(10.0), Inches(0.7))
    tf_f = foot.text_frame
    p = tf_f.paragraphs[0]
    p.text = "Auteur : Joseph Zacharie  |  Dépôt : e:\\git\\JZacharie\\Pylos  |  Version : 2.0 (Rust Workspace)"
    p.font.name = FONT_BODY
    p.font.size = Pt(11)
    p.font.color.rgb = COLOR_TEXT_MUTED

    # =========================================================================
    # SLIDE 2: Vision, Contexte & Rupture Technologique (Inspiré 4 Quadrants)
    # =========================================================================
    s2 = prs.slides.add_slide(blank_layout)
    apply_header(s2, "Vision Stratégique : Pourquoi Pylos ?", "CONPlayer & STRATÉGIE", 2)

    # Subtitle narrative
    sub_box = s2.shapes.add_textbox(Inches(0.8), Inches(1.4), Inches(11.7), Inches(0.6))
    tf_sub = sub_box.text_frame
    tf_sub.word_wrap = True
    p = tf_sub.paragraphs[0]
    p.text = "Pylos remplace les proxys d'IA traditionnels et lents (Python/Node/Bifrost) par une passerelle d'entreprise unifiée en Rust, apportant zéro compromis entre ultra-haute vitesse et contrôle budgétaire strict."
    p.font.name = FONT_BODY
    p.font.size = Pt(12)
    p.font.color.rgb = COLOR_TEXT_MUTED

    # 4 Quadrants (Green, Burgundy, Navy, Orange)
    quads = [
        ("A", "Performance & Sûreté Mémoire", COLOR_GREEN, [
            "Écrit à 100% en Rust avec runtime asynchrone Tokio et framework Axum.",
            "Surcoût d'interception réseau minimal (< 2 ms par requête).",
            "Zéro fuite mémoire, zéro Garbage Collection pauses sous forte charge.",
            "Traitement natif unifié des flux Server-Sent Events (SSE)."
        ]),
        ("B", "Gouvernance FinOps & Budgets", COLOR_BURGUNDY, [
            "Clés virtuelles applicatives 'sk-pylos-*' isolées par équipe ou projet.",
            "Suivi de consommation en USD en temps réel avec plafonds hebdomadaires/mensuels.",
            "Limites de débit strictes (RPM / TPM) pour prévenir les abus ou emballements.",
            "Coupure automatique (Hard Cap) ou notification d'alerte (Soft Cap)."
        ]),
        ("C", "Résilience & Continuité de Service", COLOR_NAVY, [
            "Routage intelligent dynamique et basculement automatique (Multi-Provider Fallback).",
            "Circuit Breaker intégré pour isoler un fournisseur défaillant sans bloquer le client.",
            "Stratégie de réessai avec recul exponentiel et gigue aléatoire (jitter).",
            "Mise à jour à chaud de la configuration sans redémarrer le serveur."
        ]),
        ("D", "Sécurité & Confidentialité Entreprise", COLOR_ORANGE, [
            "Authentification Google OAuth (SSO) pour l'UI avec secours PYLOS_ADMIN_KEY.",
            "Masquage dynamique des données personnelles sensibles (PII / regex).",
            "Proxy de contexte MCP (Model Context Protocol) avec listes de contrôle d'accès (ACL).",
            "Stockage sécurisé : clé secrète visible une seule fois à la création (obfuscation DB)."
        ])
    ]

    card_w = Inches(5.65)
    card_h = Inches(2.15)
    coords = [
        (Inches(0.8), Inches(2.1)),
        (Inches(6.8), Inches(2.1)),
        (Inches(0.8), Inches(4.5)),
        (Inches(6.8), Inches(4.5))
    ]

    for (letter, title, color, points), (x, y) in zip(quads, coords):
        c = add_card(s2, x, y, card_w, card_h, bg_color=COLOR_CARD_BG, border_color=COLOR_BORDER)
        
        # Circle badge with letter
        circle = s2.shapes.add_shape(MSO_SHAPE.OVAL, x + Inches(0.2), y + Inches(0.2), Inches(0.45), Inches(0.45))
        circle.fill.solid()
        circle.fill.fore_color.rgb = color
        circle.line.fill.background()
        tf_c = circle.text_frame
        tf_c.vertical_anchor = MSO_ANCHOR.MIDDLE
        p_c = tf_c.paragraphs[0]
        p_c.alignment = PP_ALIGN.CENTER
        p_c.text = letter
        p_c.font.name = FONT_TITLE
        p_c.font.size = Pt(13)
        p_c.font.bold = True
        p_c.font.color.rgb = COLOR_TEXT_LIGHT

        # Card Title
        tb_qt = s2.shapes.add_textbox(x + Inches(0.75), y + Inches(0.18), card_w - Inches(0.9), Inches(0.4))
        tf_qt = tb_qt.text_frame
        p_qt = tf_qt.paragraphs[0]
        p_qt.text = title
        p_qt.font.name = FONT_TITLE
        p_qt.font.size = Pt(13)
        p_qt.font.bold = True
        p_qt.font.color.rgb = color

        # Bullets
        tb_b = s2.shapes.add_textbox(x + Inches(0.3), y + Inches(0.68), card_w - Inches(0.5), card_h - Inches(0.8))
        tf_b = tb_b.text_frame
        tf_b.word_wrap = True
        tf_b.margin_left = tf_b.margin_right = tf_b.margin_top = tf_b.margin_bottom = 0
        for i, pt_text in enumerate(points):
            p = tf_b.paragraphs[0] if i == 0 else tf_b.add_paragraph()
            p.text = f"•  {pt_text}"
            p.font.name = FONT_BODY
            p.font.size = Pt(9.5)
            p.font.color.rgb = COLOR_TEXT_DARK
            p.space_before = Pt(2)

    # =========================================================================
    # SLIDE 3: Architecture Hexagonale & Découpage Système
    # =========================================================================
    s3 = prs.slides.add_slide(blank_layout)
    apply_header(s3, "Architecture Hexagonale : 4 Crates Rust & Frontend React 19", "INGÉNIERIE LOGICIELLE", 3)

    # Explanation banner
    sub_box = s3.shapes.add_textbox(Inches(0.8), Inches(1.35), Inches(11.7), Inches(0.5))
    tf_sub = sub_box.text_frame
    tf_sub.word_wrap = True
    p = tf_sub.paragraphs[0]
    p.text = "Structure en Ports & Adaptateurs isolant rigoureusement le domaine métier, l'infrastructure réseau et les protocoles d'accès."
    p.font.name = FONT_BODY
    p.font.size = Pt(11)
    p.font.color.rgb = COLOR_TEXT_MUTED

    crates_data = [
        ("pylos-server", "Couche Présentation & Web", COLOR_NAVY, [
            "Axum 0.7 & Tokio 1.36 runtime",
            "Endpoints compatibles OpenAI (/v1/*)",
            "Proxy MCP (Model Context Protocol)",
            "Streaming SSE & WebSockets",
            "Middlewares Auth, Métriques & Logs"
        ]),
        ("pylos-application", "Cas d'Utilisation & Orchestration", COLOR_GREEN, [
            "InferenceOrchestrator & Smart-Routing",
            "Dynamic Batching (OpenAI Batch & Coalescing)",
            "Plugins RAG, Semantic & Prefix Cache",
            "Gestionnaire de mémoire Memgraph Bolt",
            "ConfigStore avec Hot-Reload dynamique"
        ]),
        ("pylos-infrastructure", "Adaptateurs & Fournisseurs", COLOR_BURGUNDY, [
            "20+ adaptateurs de LLM (OpenAI, Anthropic...)",
            "AWS Bedrock SDK (Assume-Role STS)",
            "Persistance PostgreSQL (sqlx) & SQLite",
            "Client Qdrant Vectoriel & HTTP Clients",
            "Chiffrement & Obfuscation des secrets"
        ]),
        ("pylos-core", "Domaine Métier & Ports", COLOR_ORANGE, [
            "Entités et règles métier pures (Zéro I/O)",
            "Schémas JSON OpenAI Chat & Embeddings",
            "Traits Provider & LlmPlugin",
            "Gestion des erreurs hiérarchiques (PylosError)",
            "Définitions de VirtualKey, Budget, ModelCatalog"
        ])
    ]

    crate_w = Inches(2.75)
    crate_h = Inches(3.8)
    for i, (name, role, color, features) in enumerate(crates_data):
        cx = Inches(0.8) + i * Inches(2.95)
        cy = Inches(1.9)
        c = add_card(s3, cx, cy, crate_w, crate_h, bg_color=COLOR_CARD_BG, border_color=color)

        # Top banner for crate
        t_bar = s3.shapes.add_shape(MSO_SHAPE.RECTANGLE, cx, cy, crate_w, Inches(0.6))
        t_bar.fill.solid()
        t_bar.fill.fore_color.rgb = color
        t_bar.line.fill.background()
        tf_tb = t_bar.text_frame
        tf_tb.vertical_anchor = MSO_ANCHOR.MIDDLE
        p_cname = tf_tb.paragraphs[0]
        p_cname.alignment = PP_ALIGN.CENTER
        p_cname.text = name
        p_cname.font.name = FONT_CODE
        p_cname.font.size = Pt(12)
        p_cname.font.bold = True
        p_cname.font.color.rgb = COLOR_TEXT_LIGHT

        # Subtitle role
        s_box = s3.shapes.add_textbox(cx + Inches(0.1), cy + Inches(0.65), crate_w - Inches(0.2), Inches(0.35))
        tf_s = s_box.text_frame
        p_s = tf_s.paragraphs[0]
        p_s.alignment = PP_ALIGN.CENTER
        p_s.text = role
        p_s.font.name = FONT_TITLE
        p_s.font.size = Pt(9.5)
        p_s.font.bold = True
        p_s.font.color.rgb = COLOR_TEXT_DARK

        # Divider
        div = s3.shapes.add_shape(MSO_SHAPE.RECTANGLE, cx + Inches(0.2), cy + Inches(1.05), crate_w - Inches(0.4), Inches(0.01))
        div.fill.solid()
        div.fill.fore_color.rgb = COLOR_BORDER
        div.line.fill.background()

        # Features list
        f_box = s3.shapes.add_textbox(cx + Inches(0.15), cy + Inches(1.15), crate_w - Inches(0.3), Inches(2.5))
        tf_f = f_box.text_frame
        tf_f.word_wrap = True
        for j, feat in enumerate(features):
            p = tf_f.paragraphs[0] if j == 0 else tf_f.add_paragraph()
            p.text = f"✔  {feat}"
            p.font.name = FONT_BODY
            p.font.size = Pt(9.5)
            p.font.color.rgb = COLOR_TEXT_DARK
            p.space_before = Pt(3)

    # Bottom Banner for Frontend & Cluster
    b_banner = add_card(s3, Inches(0.8), Inches(5.9), Inches(11.7), Inches(0.85), bg_color=COLOR_CARD_MUTED, border_color=COLOR_BORDER)
    bb_box = s3.shapes.add_textbox(Inches(1.0), Inches(5.95), Inches(11.3), Inches(0.75))
    tf_bb = bb_box.text_frame
    tf_bb.word_wrap = True
    p = tf_bb.paragraphs[0]
    p.text = "INTERFACE WEB DÉDIÉE & GITOPS :"
    p.font.name = FONT_TITLE
    p.font.size = Pt(10)
    p.font.bold = True
    p.font.color.rgb = COLOR_NAVY

    p2 = tf_bb.add_paragraph()
    p2.text = "Dashboard moderne React 19 + TypeScript + Vite 8 + TailwindCSS 3. Déploiement GitOps Kubernetes sur le cluster 'jo3' orchestré par ArgoCD, persistance PostgreSQL haute disponibilité (cluster-pg-1) et secrets pilotés par HashiCorp Vault."
    p2.font.name = FONT_BODY
    p2.font.size = Pt(9.5)
    p2.font.color.rgb = COLOR_TEXT_DARK

    # =========================================================================
    # SLIDE 4: Ce qui est FAIT — Inférence, Modèles & Haute Disponibilité
    # =========================================================================
    s4 = prs.slides.add_slide(blank_layout)
    apply_header(s4, "Ce qui est Fait : Moteur d'Inférence & 20+ Fournisseurs LLM", "BILAN TECHNIQUE : INFERENCE", 4)

    # Left Column: Supported Providers & Endpoints (Card)
    add_card(s4, Inches(0.8), Inches(1.4), Inches(5.65), Inches(5.35), bg_color=COLOR_CARD_BG, border_color=COLOR_BORDER)
    add_badge(s4, Inches(1.1), Inches(1.65), Inches(2.5), Inches(0.35), "FOURNISSEURS & PROTOCOLES", COLOR_GREEN, font_size=10)

    tb = s4.shapes.add_textbox(Inches(1.1), Inches(2.15), Inches(5.1), Inches(4.4))
    tf = tb.text_frame
    tf.word_wrap = True
    
    items_left = [
        ("Fournisseurs Connectés (+20)", "OpenAI, Anthropic, AWS Bedrock (STS), Google Gemini, DeepSeek, Groq, Ollama, OpenRouter, Mistral, Cohere, Perplexity..."),
        ("Endpoints Unifiés OpenAI", "Remplacement transparent sans altération de code client :\n• POST /v1/chat/completions (Unaire & Streaming SSE)\n• POST /v1/embeddings (Calcul vectoriel unifié)\n• POST /v1/images/generations\n• GET /v1/models (Catalogue dynamique agrégé)"),
        ("Routage Dynamique & Poids", "Distribution pondérée du trafic (Load Balancing), bascule selon disponibilité et règles CEL (Common Expression Language)."),
        ("Dynamic Batching Avancé", "Double modalité : Coalescing transparent via boucle tokio::select! + support complet de l'OpenAI Batch API (JSONL, upload, polling).")
    ]
    for i, (head, desc) in enumerate(items_left):
        p_h = tf.paragraphs[0] if i == 0 else tf.add_paragraph()
        p_h.text = f"• {head}"
        p_h.font.name = FONT_TITLE
        p_h.font.size = Pt(11)
        p_h.font.bold = True
        p_h.font.color.rgb = COLOR_GREEN
        p_h.space_before = Pt(4)

        p_d = tf.add_paragraph()
        p_d.text = desc
        p_d.font.name = FONT_BODY
        p_d.font.size = Pt(9.5)
        p_d.font.color.rgb = COLOR_TEXT_DARK

    # Right Column: Resilience & Circuit Breaker (Card)
    add_card(s4, Inches(6.8), Inches(1.4), Inches(5.65), Inches(5.35), bg_color=COLOR_CARD_BG, border_color=COLOR_BORDER)
    add_badge(s4, Inches(7.1), Inches(1.65), Inches(2.8), Inches(0.35), "RÉSILIENCE & HAUTE DISPONIBILITÉ", COLOR_BURGUNDY, font_size=10)

    tb_r = s4.shapes.add_textbox(Inches(7.1), Inches(2.15), Inches(5.1), Inches(4.4))
    tf_r = tb_r.text_frame
    tf_r.word_wrap = True

    items_right = [
        ("Cascade de Secours (Fallback Multi-Fournisseur)", "Si le fournisseur primaire renvoie une erreur (429 Quota Exceeded, 500/503 Downtime), la requête bascule instantanément sur le modèle équivalent de repli configuré."),
        ("Disjoncteur Intelligent (Circuit Breaker)", "Surveillance des taux d'échec par fournisseur. En cas de défaillance répétée, le circuit s'ouvre pour éviter de saturer le réseau et préserver la latence globale."),
        ("Retry Exponentiel & Jitter", "Réessais automatiques avec délai progressif configurable (ex: 500ms initial, 5000ms max) pour lisser les congestions transitoires."),
        ("Structured Output Auto-Correction", "Validation du schéma JSON retourné par le LLM. En cas d'erreur de conformité, Pylos réinjecte le message d'erreur dans une boucle d'auto-correction LLM avant de renvoyer le résultat.")
    ]
    for i, (head, desc) in enumerate(items_right):
        p_h = tf_r.paragraphs[0] if i == 0 else tf_r.add_paragraph()
        p_h.text = f"• {head}"
        p_h.font.name = FONT_TITLE
        p_h.font.size = Pt(11)
        p_h.font.bold = True
        p_h.font.color.rgb = COLOR_BURGUNDY
        p_h.space_before = Pt(4)

        p_d = tf_r.add_paragraph()
        p_d.text = desc
        p_d.font.name = FONT_BODY
        p_d.font.size = Pt(9.5)
        p_d.font.color.rgb = COLOR_TEXT_DARK

    # =========================================================================
    # SLIDE 5: Ce qui est FAIT — Caching & Économie de Tokens (Inspiré 2 étapes)
    # =========================================================================
    s5 = prs.slides.add_slide(blank_layout)
    apply_header(s5, "Ce qui est Fait : Stratégie de Caching & Économie de Tokens", "BILAN TECHNIQUE : CACHING", 5)

    sub_box = s5.shapes.add_textbox(Inches(0.8), Inches(1.35), Inches(11.7), Inches(0.5))
    tf_sub = sub_box.text_frame
    tf_sub.word_wrap = True
    p = tf_sub.paragraphs[0]
    p.text = "Pylos intègre deux étages de mise en cache ultra-performants permettant d'économiser jusqu'à 100% des tokens LLM et d'offrir une latence de réponse < 5ms."
    p.font.name = FONT_BODY
    p.font.size = Pt(11)
    p.font.color.rgb = COLOR_TEXT_MUTED

    # Left: Prefix Cache TinyLFU
    add_card(s5, Inches(0.8), Inches(1.9), Inches(5.65), Inches(4.8), bg_color=COLOR_CARD_BG, border_color=COLOR_GREEN)
    
    # Header badge
    add_badge(s5, Inches(1.1), Inches(2.1), Inches(3.2), Inches(0.35), "1. CACHE DE PRÉFIXE EN MÉMOIRE", COLOR_GREEN, font_size=10)

    tb_pc = s5.shapes.add_textbox(Inches(1.1), Inches(2.6), Inches(5.1), Inches(3.9))
    tf_pc = tb_pc.text_frame
    tf_pc.word_wrap = True

    pts_pc = [
        ("Moteur & Algorithme", "Bibliothèque Rust 'moka' avec politique d'éviction haute performance TinyLFU (Tiny Least Frequently Used)."),
        ("Génération de Rolling Keys", "Génère des clés de préfixes itératifs sur l'historique de conversation (ex: 'model|System:...|User:Hello|')."),
        ("Recherche Décroissante (Reverse)", "Pylos recherche d'abord le préfixe le plus long disponible pour maximiser le gain de contexte."),
        ("Gain Financier & Temporel", "Sur hit exact : court-circuite totalement l'appel au LLM. Économie de 100% des tokens prompt & completion et réponse en ~1ms.")
    ]
    for i, (h, d) in enumerate(pts_pc):
        p = tf_pc.paragraphs[0] if i == 0 else tf_pc.add_paragraph()
        p.text = f"✔ {h} :"
        p.font.name = FONT_TITLE
        p.font.size = Pt(10.5)
        p.font.bold = True
        p.font.color.rgb = COLOR_GREEN
        p.space_before = Pt(3)

        p2 = tf_pc.add_paragraph()
        p2.text = d
        p2.font.name = FONT_BODY
        p2.font.size = Pt(9.5)
        p2.font.color.rgb = COLOR_TEXT_DARK

    # Right: Semantic Cache Qdrant
    add_card(s5, Inches(6.8), Inches(1.9), Inches(5.65), Inches(4.8), bg_color=COLOR_CARD_BG, border_color=COLOR_ORANGE)
    add_badge(s5, Inches(7.1), Inches(2.1), Inches(3.2), Inches(0.35), "2. CACHE SÉMANTIQUE VECTORIEL", COLOR_ORANGE, font_size=10)

    tb_sc = s5.shapes.add_textbox(Inches(7.1), Inches(2.6), Inches(5.1), Inches(3.9))
    tf_sc = tb_sc.text_frame
    tf_sc.word_wrap = True

    pts_sc = [
        ("Moteur Vectoriel Qdrant", "Calcul d'embedding direct via provider interne (zéro boucle HTTP loopback) et stockage indexé dans Qdrant."),
        ("Similarité Cosinus", "Recherche dans l'espace vectoriel avec seuil de similarité configurable (ex: seuil cosinus à 0.92 ou 0.95)."),
        ("Compréhension d'Intention", "Détecte les questions équivalentes même si le prompt est formulé avec d'autres mots (ex: 'Quels sont vos tarifs ?' vs 'Combien coûte votre service ?')."),
        ("Élimination des Doublons LLM", "Renvoie instantanément la réponse mise en cache sans solliciter le modèle distant.")
    ]
    for i, (h, d) in enumerate(pts_sc):
        p = tf_sc.paragraphs[0] if i == 0 else tf_sc.add_paragraph()
        p.text = f"✔ {h} :"
        p.font.name = FONT_TITLE
        p.font.size = Pt(10.5)
        p.font.bold = True
        p.font.color.rgb = COLOR_ORANGE
        p.space_before = Pt(3)

        p2 = tf_sc.add_paragraph()
        p2.text = d
        p2.font.name = FONT_BODY
        p2.font.size = Pt(9.5)
        p2.font.color.rgb = COLOR_TEXT_DARK

    # =========================================================================
    # SLIDE 6: Ce qui est FAIT — RAG Avancé & Graphe de Mémoire Memgraph
    # =========================================================================
    s6 = prs.slides.add_slide(blank_layout)
    apply_header(s6, "Ce qui est Fait : Graphe de Mémoire & RAG Augmenté", "BILAN TECHNIQUE : DATA & IA", 6)

    # 3 Column cards (Memgraph Bolt, Pipeline RAG, CRAG & HyDE)
    col_w = Inches(3.7)
    col_h = Inches(5.2)

    data_rag = [
        ("Graphe de Mémoire (Memgraph)", COLOR_NAVY, "Protocole Bolt & Cypher (neo4rs)", [
            "Mémoire à long terme partagée cross-agents et persistante.",
            "Pre-hook : extraction des entités liées à la VirtualKey et injection dynamique dans le prompt système.",
            "Post-hook : analyse du flux LLM pour détecter les balises <memory>Entité|RELATION|Valeur</memory>.",
            "Supporte l'extraction aussi bien en mode unaire qu'en streaming SSE actif.",
            "Requêtes Cypher MERGE automatiques dans la base Memgraph."
        ]),
        ("Pipeline RAG Vectoriel", COLOR_GREEN, "Qdrant Vector Store & Chunking", [
            "Chunking Parent-Child : découpage en blocs parents (~1000 cars) et sous-blocs enfants (~150 cars) avec overlap.",
            "Recherche vectorielle fine sur l'enfant, restitution du contexte complet via le bloc parent.",
            "Modèles configurables dynamiquement dans pylos.json (pylos_model, embedding_model).",
            "Templates de prompt système configurables avec balise {context}.",
            "Pas de HTTP loopback : appels embeddings directs."
        ]),
        ("RAG Avancé : HyDE & CRAG", COLOR_BURGUNDY, "Corrective RAG & Query Expansion", [
            "Query Expansion : génération automatique de reformulations de questions via LLM pour balayer Qdrant.",
            "HyDE (Hypothetical Document Embeddings) : génère une réponse idéale théorique pour guider le vecteur de recherche.",
            "CRAG (Corrective RAG) : auto-évaluation du score de confiance des résultats vectoriels.",
            "Recherche Web de Secours : si max_score < seuil (ex: 0.75), bascule sur l'API Tavily.",
            "Fallback gracieux en cas d'indisponibilité externe."
        ])
    ]

    for i, (title, color, sub, bullets) in enumerate(data_rag):
        cx = Inches(0.8) + i * Inches(4.0)
        cy = Inches(1.5)
        c = add_card(s6, cx, cy, col_w, col_h, bg_color=COLOR_CARD_BG, border_color=color)

        # Header block
        hb = s6.shapes.add_shape(MSO_SHAPE.RECTANGLE, cx, cy, col_w, Inches(0.7))
        hb.fill.solid()
        hb.fill.fore_color.rgb = color
        hb.line.fill.background()
        tf_hb = hb.text_frame
        tf_hb.vertical_anchor = MSO_ANCHOR.MIDDLE
        p = tf_hb.paragraphs[0]
        p.alignment = PP_ALIGN.CENTER
        p.text = title
        p.font.name = FONT_TITLE
        p.font.size = Pt(11)
        p.font.bold = True
        p.font.color.rgb = COLOR_TEXT_LIGHT

        # Subtitle
        sb = s6.shapes.add_textbox(cx + Inches(0.1), cy + Inches(0.75), col_w - Inches(0.2), Inches(0.4))
        tf_sb = sb.text_frame
        p = tf_sb.paragraphs[0]
        p.alignment = PP_ALIGN.CENTER
        p.text = sub
        p.font.name = FONT_BODY
        p.font.size = Pt(9)
        p.font.bold = True
        p.font.color.rgb = COLOR_TEXT_MUTED

        # Divider
        div = s6.shapes.add_shape(MSO_SHAPE.RECTANGLE, cx + Inches(0.2), cy + Inches(1.15), col_w - Inches(0.4), Inches(0.01))
        div.fill.solid()
        div.fill.fore_color.rgb = COLOR_BORDER
        div.line.fill.background()

        # Bullets
        bb = s6.shapes.add_textbox(cx + Inches(0.15), cy + Inches(1.25), col_w - Inches(0.3), Inches(3.8))
        tf_bb = bb.text_frame
        tf_bb.word_wrap = True
        for j, b in enumerate(bullets):
            p = tf_bb.paragraphs[0] if j == 0 else tf_bb.add_paragraph()
            p.text = f"•  {b}"
            p.font.name = FONT_BODY
            p.font.size = Pt(9.5)
            p.font.color.rgb = COLOR_TEXT_DARK
            p.space_before = Pt(3)

    # =========================================================================
    # SLIDE 7: Ce qui est FAIT — Gouvernance, FinOps & Sécurité (Inspiré 4 Thermomètres)
    # =========================================================================
    s7 = prs.slides.add_slide(blank_layout)
    apply_header(s7, "Ce qui est Fait : Gouvernance FinOps, Sécurité & Dashboard", "BILAN TECHNIQUE : CONTRÔLE", 7)

    cards_gov = [
        ("Clés Virtuelles", "sk-pylos-*", COLOR_NAVY, [
            "Affichage unique à la création (obfuscation 'sk-py****').",
            "Attribution fine par équipe ou organisation.",
            "Filtrage strict des modèles et fournisseurs autorisés.",
            "Révocation instantanée en un clic."
        ]),
        ("Budgets FinOps", "Plafonds en $ USD", COLOR_BURGUNDY, [
            "Suivi continu des tokens d'entrée, de sortie et d'images.",
            "Fenêtres glissantes : heure, jour, semaine, mois.",
            "Soft Cap : alerte webhook sans bloquer l'usage.",
            "Hard Cap : blocage immédiat 429 lors du dépassement."
        ]),
        ("Garde-Fous & MCP", "Sécurité des flux", COLOR_GREEN, [
            "Garde-fous PII : masquage automatique des données sensibles.",
            "Proxy MCP : connexion dynamique aux serveurs d'outils tiers.",
            "Contrôle d'accès MCP (ACL) par clé et équipe.",
            "Rate limiting strict sur les endpoints MCP (429)."
        ]),
        ("Dashboard React 19", "Console d'administration", COLOR_ORANGE, [
            "16 pages opérationnelles sans aucun placeholder.",
            "API Playground interactif pour tester les modèles.",
            "Graphiques Recharts de consommation et coûts.",
            "Éditeur de règles de routage CEL et logs d'audit."
        ])
    ]

    card_w7 = Inches(2.75)
    card_h7 = Inches(5.1)
    for i, (title, sub, color, points) in enumerate(cards_gov):
        cx = Inches(0.8) + i * Inches(2.95)
        cy = Inches(1.5)
        c = add_card(s7, cx, cy, card_w7, card_h7, bg_color=COLOR_CARD_BG, border_color=color)

        # Top pill badge
        add_badge(s7, cx + Inches(0.2), cy + Inches(0.2), card_w7 - Inches(0.4), Inches(0.4), title, color, font_size=11)

        # Subtitle box
        sb = s7.shapes.add_textbox(cx + Inches(0.1), cy + Inches(0.65), card_w7 - Inches(0.2), Inches(0.35))
        tf_sb = sb.text_frame
        p = tf_sb.paragraphs[0]
        p.alignment = PP_ALIGN.CENTER
        p.text = sub
        p.font.name = FONT_CODE
        p.font.size = Pt(10)
        p.font.color.rgb = COLOR_TEXT_MUTED

        # Divider
        div = s7.shapes.add_shape(MSO_SHAPE.RECTANGLE, cx + Inches(0.2), cy + Inches(1.05), card_w7 - Inches(0.4), Inches(0.01))
        div.fill.solid()
        div.fill.fore_color.rgb = COLOR_BORDER
        div.line.fill.background()

        # Bullets
        bb = s7.shapes.add_textbox(cx + Inches(0.15), cy + Inches(1.2), card_w7 - Inches(0.3), Inches(3.7))
        tf_bb = bb.text_frame
        tf_bb.word_wrap = True
        for j, pt in enumerate(points):
            p = tf_bb.paragraphs[0] if j == 0 else tf_bb.add_paragraph()
            p.text = f"✔  {pt}"
            p.font.name = FONT_BODY
            p.font.size = Pt(9.5)
            p.font.color.rgb = COLOR_TEXT_DARK
            p.space_before = Pt(4)

    # =========================================================================
    # SLIDE 8: Ce qui RESTE À FAIRE — Roadmap & Validation SIT
    # =========================================================================
    s8 = prs.slides.add_slide(blank_layout)
    apply_header(s8, "Ce qui Reste à Faire : Roadmap vers la Production", "ROADMAP & OBJECTIFS", 8)

    # Context alert box
    alert_box = add_card(s8, Inches(0.8), Inches(1.4), Inches(11.7), Inches(0.85), bg_color=RGBColor(254, 243, 199), border_color=COLOR_ORANGE)
    ab_tb = s8.shapes.add_textbox(Inches(1.0), Inches(1.45), Inches(11.3), Inches(0.75))
    tf_ab = ab_tb.text_frame
    tf_ab.word_wrap = True
    p = tf_ab.paragraphs[0]
    p.text = "STATUT DU PROJET : NON PRÊT POUR LA PRODUCTION (NON-PRODUCTION READY)"
    p.font.name = FONT_TITLE
    p.font.size = Pt(11)
    p.font.bold = True
    p.font.color.rgb = COLOR_BURGUNDY

    p2 = tf_ab.add_paragraph()
    p2.text = "Bien que les tests techniques unitaires et de compilation passent à 100% (95/95 tests Rust, Clippy sans warnings), les tests d'intégration système (SIT) en conditions réelles doivent être finalisés pour valider définitivement le déploiement en production."
    p2.font.name = FONT_BODY
    p2.font.size = Pt(9.5)
    p2.font.color.rgb = COLOR_TEXT_DARK

    # 4 Roadmap Steps (Inspiration Tree / Milestone Pipeline)
    steps = [
        ("01", "Campagne de Tests d'Intégration Système (SIT)", COLOR_BURGUNDY, "Priorité Immédiate", [
            "Validation de bout en bout des flux d'inférence sous charge réelle avec clés tierces activées.",
            "Tests de bascule automatique (chaos testing : simulation de pannes d'API OpenAI/Anthropic).",
            "Vérification de la cohérence transactionnelle PostgreSQL sous concurrence de VirtualKeys."
        ]),
        ("02", "Stress Testing & Benchmarking p95/p99", COLOR_NAVY, "Performance & Échelle", [
            "Évaluation formelle de débit sous 5 000 à 10 000 requêtes/seconde.",
            "Mesure instrumentée des latences p50, p95 et p99 avec streaming actif.",
            "Optimisation fine du pool de connexions SQLx et de la rétention mémoire TinyLFU."
        ]),
        ("03", "Connecteurs Vectoriels Additionnels", COLOR_GREEN, "Écosystème Vectoriel", [
            "Support de PostgreSQL avec extension pgvector (alternative native à Qdrant sans serveur dédié).",
            "Gestion multi-collections dans Qdrant avec règles de partitionnement par organisation.",
            "Enrichissement des politiques de RAG hybride (mélange recherche dense et recherche lexicale BM25)."
        ]),
        ("04", "Observabilité Avancée & FinOps Live OpenObserve", COLOR_ORANGE, "Exploitation & Supervision", [
            "Finalisation de la synchronisation automatique des dashboards v8 dans jo3/ (GitOps).",
            "Alertes prédictives FinOps par canal Slack/Teams basées sur la dérive d'usage.",
            "Rapports d'audit automatisés consolidant les coûts réels versus budgets attribués."
        ])
    ]

    card_w8 = Inches(5.65)
    card_h8 = Inches(2.2)
    coords8 = [
        (Inches(0.8), Inches(2.45)),
        (Inches(6.8), Inches(2.45)),
        (Inches(0.8), Inches(4.85)),
        (Inches(6.8), Inches(4.85))
    ]

    for (num, title, color, tag, bullets), (x, y) in zip(steps, coords8):
        c = add_card(s8, x, y, card_w8, card_h8, bg_color=COLOR_CARD_BG, border_color=COLOR_BORDER)
        
        # Step badge
        add_badge(s8, x + Inches(0.2), y + Inches(0.2), Inches(0.55), Inches(0.35), num, color, font_size=11)

        # Title & Tag
        tb_st = s8.shapes.add_textbox(x + Inches(0.85), y + Inches(0.18), card_w8 - Inches(1.0), Inches(0.4))
        tf_st = tb_st.text_frame
        p_st = tf_st.paragraphs[0]
        p_st.text = title
        p_st.font.name = FONT_TITLE
        p_st.font.size = Pt(11)
        p_st.font.bold = True
        p_st.font.color.rgb = color

        p_tag = tf_st.add_paragraph()
        p_tag.text = tag.upper()
        p_tag.font.name = FONT_BODY
        p_tag.font.size = Pt(8.5)
        p_tag.font.color.rgb = COLOR_TEXT_MUTED

        # Bullets
        tb_b = s8.shapes.add_textbox(x + Inches(0.2), y + Inches(0.7), card_w8 - Inches(0.4), card_h8 - Inches(0.8))
        tf_b = tb_b.text_frame
        tf_b.word_wrap = True
        tf_b.margin_left = tf_b.margin_right = tf_b.margin_top = tf_b.margin_bottom = 0
        for k, pt in enumerate(bullets):
            p = tf_b.paragraphs[0] if k == 0 else tf_b.add_paragraph()
            p.text = f"•  {pt}"
            p.font.name = FONT_BODY
            p.font.size = Pt(9)
            p.font.color.rgb = COLOR_TEXT_DARK
            p.space_before = Pt(2)

    # =========================================================================
    # SLIDE 9: Comment l'Utiliser — Côté Développeur IA
    # =========================================================================
    s9 = prs.slides.add_slide(blank_layout)
    apply_header(s9, "Comment l'Utiliser : Intégration Développeur & SDK", "GUIDE D'UTILISATION : DEVELOPPEURS", 9)

    # Intro note
    sub_box = s9.shapes.add_textbox(Inches(0.8), Inches(1.35), Inches(11.7), Inches(0.5))
    tf_sub = sub_box.text_frame
    tf_sub.word_wrap = True
    p = tf_sub.paragraphs[0]
    p.text = "Remplacement 'Drop-in' 100% compatible OpenAI : modifiez uniquement la base_url et votre clé sk-pylos-* sans altérer une seule ligne de logique métier !"
    p.font.name = FONT_BODY
    p.font.size = Pt(11.5)
    p.font.color.rgb = COLOR_GREEN

    # Left: Python SDK Example
    add_card(s9, Inches(0.8), Inches(1.9), Inches(5.65), Inches(4.8), bg_color=COLOR_CODE_BG, border_color=None)
    add_badge(s9, Inches(1.0), Inches(2.05), Inches(2.2), Inches(0.3), "PYTHON (SDK OPENAI)", COLOR_NAVY, font_size=9.5)

    code_py = (
        "import os\n"
        "from openai import OpenAI\n\n"
        "# 1. Instancier le client OpenAI pointant vers Pylos\n"
        "client = OpenAI(\n"
        "    base_url=\"http://localhost:3000/v1\",\n"
        "    api_key=\"sk-pylos-marketing-production-key\"\n"
        ")\n\n"
        "# 2. Appel unifié multi-fournisseur transparent\n"
        "response = client.chat.completions.create(\n"
        "    model=\"claude-sonnet-5-5\",  # ou gemini-3.8-flash, gpt-4o\n"
        "    messages=[\n"
        "        {\"role\": \"system\", \"content\": \"Tu es un analyste Pylos.\"},\n"
        "        {\"role\": \"user\", \"content\": \"Résume les logs d'aujourd'hui.\"}\n"
        "    ],\n"
        "    temperature=0.7,\n"
        "    stream=True  # Support du streaming SSE natif\n"
        ")\n\n"
        "for chunk in response:\n"
        "    print(chunk.choices[0].delta.content or \"\", end=\"\")"
    )

    tb_cpy = s9.shapes.add_textbox(Inches(1.0), Inches(2.45), Inches(5.25), Inches(4.1))
    tf_cpy = tb_cpy.text_frame
    tf_cpy.word_wrap = True
    tf_cpy.margin_left = tf_cpy.margin_right = tf_cpy.margin_top = tf_cpy.margin_bottom = 0
    p = tf_cpy.paragraphs[0]
    p.text = code_py
    p.font.name = FONT_CODE
    p.font.size = Pt(8.5)
    p.font.color.rgb = COLOR_CODE_TEXT

    # Right: cURL & MCP Tool Example
    add_card(s9, Inches(6.8), Inches(1.9), Inches(5.65), Inches(4.8), bg_color=COLOR_CODE_BG, border_color=None)
    add_badge(s9, Inches(7.0), Inches(2.05), Inches(2.0), Inches(0.3), "REQUÊTE HTTP CURL", COLOR_ORANGE, font_size=9.5)

    code_curl = (
        "# Requête HTTP directe via cURL\n"
        "curl -X POST http://localhost:3000/v1/chat/completions \\\n"
        "  -H \"Content-Type: application/json\" \\\n"
        "  -H \"Authorization: Bearer sk-pylos-app-token\" \\\n"
        "  -d '{\n"
        "    \"model\": \"gemini-3.8-flash\",\n"
        "    \"messages\": [{\"role\": \"user\", \"content\": \"Calculer le budget IA\"}],\n"
        "    \"max_tokens\": 250\n"
        "  }'\n\n"
        "# Appel d'outil MCP dynamique (Model Context Protocol)\n"
        "curl -X POST http://localhost:3000/mcp/tools/call \\\n"
        "  -H \"Authorization: Bearer sk-pylos-agent-key\" \\\n"
        "  -H \"Content-Type: application/json\" \\\n"
        "  -d '{\n"
        "    \"server\": \"cluster-jo3-observability\",\n"
        "    \"tool\": \"GetLatestTraces\",\n"
        "    \"arguments\": {\"service\": \"pylos-server\"}\n"
        "  }'"
    )

    tb_ccurl = s9.shapes.add_textbox(Inches(7.0), Inches(2.45), Inches(5.25), Inches(4.1))
    tf_ccurl = tb_ccurl.text_frame
    tf_ccurl.word_wrap = True
    tf_ccurl.margin_left = tf_ccurl.margin_right = tf_ccurl.margin_top = tf_ccurl.margin_bottom = 0
    p = tf_ccurl.paragraphs[0]
    p.text = code_curl
    p.font.name = FONT_CODE
    p.font.size = Pt(8.5)
    p.font.color.rgb = COLOR_CODE_TEXT

    # =========================================================================
    # SLIDE 10: Comment l'Utiliser — Côté Administrateur & Déploiement
    # =========================================================================
    s10 = prs.slides.add_slide(blank_layout)
    apply_header(s10, "Comment l'Utiliser : Déploiement & Administration", "GUIDE D'UTILISATION : OPS & ADMIN", 10)

    ops_steps = [
        ("1", "Démarrage & Déploiement", COLOR_NAVY, [
            "Local Dev : 'make run' ou 'docker compose up -d'.",
            "Windows : binaire natif 'pylos.exe' ou installeur 'Pylos.msi' via build-windows.ps1.",
            "Kubernetes : déploiement GitOps ArgoCD sur le cluster jo3 avec 2 réplicas stateless."
        ]),
        ("2", "Configuration Centrale", COLOR_GREEN, [
            "Fichier unifié 'pylos.json' à la racine de l'application.",
            "Déclaration sécurisée des clés d'API fournisseurs via variables d'environnement.",
            "Activation des plugins de cache (TinyLFU) et RAG vectoriel (Qdrant)."
        ]),
        ("3", "Console Web & Clés Virtuelles", COLOR_BURGUNDY, [
            "Connexion sur http://localhost:8080 via Google SSO ou clé PYLOS_ADMIN_KEY.",
            "Génération des Virtual Keys avec limitation par modèle, organisation et budget.",
            "Visualisation en direct des logs d'inférence, de la latence et des tokens consommés."
        ]),
        ("4", "Rechargement à Chaud", COLOR_ORANGE, [
            "Mise à jour des modèles et quotas sans redémarrer le gateway.",
            "Appel sécurisé : POST /config/reload.",
            "Zéro coupure de service pour les applications consommatrices actives."
        ])
    ]

    cw10 = Inches(2.75)
    ch10 = Inches(5.1)
    for i, (num, title, color, points) in enumerate(ops_steps):
        cx = Inches(0.8) + i * Inches(2.95)
        cy = Inches(1.5)
        c = add_card(s10, cx, cy, cw10, ch10, bg_color=COLOR_CARD_BG, border_color=color)

        # Step circle
        circle = s10.shapes.add_shape(MSO_SHAPE.OVAL, cx + Inches(1.1), cy + Inches(0.2), Inches(0.55), Inches(0.55))
        circle.fill.solid()
        circle.fill.fore_color.rgb = color
        circle.line.fill.background()
        tf_c = circle.text_frame
        tf_c.vertical_anchor = MSO_ANCHOR.MIDDLE
        p = tf_c.paragraphs[0]
        p.alignment = PP_ALIGN.CENTER
        p.text = num
        p.font.name = FONT_TITLE
        p.font.size = Pt(14)
        p.font.bold = True
        p.font.color.rgb = COLOR_TEXT_LIGHT

        # Step Title
        tb_t = s10.shapes.add_textbox(cx + Inches(0.1), cy + Inches(0.85), cw10 - Inches(0.2), Inches(0.4))
        tf_t = tb_t.text_frame
        p = tf_t.paragraphs[0]
        p.alignment = PP_ALIGN.CENTER
        p.text = title
        p.font.name = FONT_TITLE
        p.font.size = Pt(11)
        p.font.bold = True
        p.font.color.rgb = color

        # Divider
        div = s10.shapes.add_shape(MSO_SHAPE.RECTANGLE, cx + Inches(0.2), cy + Inches(1.3), cw10 - Inches(0.4), Inches(0.01))
        div.fill.solid()
        div.fill.fore_color.rgb = COLOR_BORDER
        div.line.fill.background()

        # Bullets
        bb = s10.shapes.add_textbox(cx + Inches(0.15), cy + Inches(1.4), cw10 - Inches(0.3), Inches(3.5))
        tf_bb = bb.text_frame
        tf_bb.word_wrap = True
        for j, pt in enumerate(points):
            p = tf_bb.paragraphs[0] if j == 0 else tf_bb.add_paragraph()
            p.text = f"•  {pt}"
            p.font.name = FONT_BODY
            p.font.size = Pt(9.5)
            p.font.color.rgb = COLOR_TEXT_DARK
            p.space_before = Pt(4)

    # =========================================================================
    # SLIDE 11: Exemple Concret — Configuration Réelle `pylos.json`
    # =========================================================================
    s11 = prs.slides.add_slide(blank_layout)
    apply_header(s11, "Exemple Concret : Configuration Unifiée 'pylos.json'", "CONFIGURATION SYSTÈME", 11)

    # Left: JSON Config code snippet
    add_card(s11, Inches(0.8), Inches(1.4), Inches(6.8), Inches(5.35), bg_color=COLOR_CODE_BG, border_color=None)
    add_badge(s11, Inches(1.0), Inches(1.55), Inches(2.2), Inches(0.3), "PYLOS.JSON (EXTRAIT)", COLOR_NAVY, font_size=9.5)

    sample_json = (
        "{\n"
        "  \"version\": 2,\n"
        "  \"server\": {\n"
        "    \"port\": 3000, \"host\": \"0.0.0.0\",\n"
        "    \"queuing\": { \"max_concurrency\": 100, \"queue_timeout_ms\": 30000 }\n"
        "  },\n"
        "  \"providers\": {\n"
        "    \"antigravity\": {\n"
        "      \"keys\": [{\n"
        "        \"name\": \"prod-key\",\n"
        "        \"value\": \"env.GEMINI_API_KEY\",\n"
        "        \"models\": [\"gemini-3.8-flash\", \"claude-sonnet-5-5\"],\n"
        "        \"weight\": 1.0\n"
        "      }],\n"
        "      \"network\": { \"timeout_secs\": 120, \"max_retries\": 2 }\n"
        "    }\n"
        "  },\n"
        "  \"plugins\": [\n"
        "    {\n"
        "      \"name\": \"prefix_cache\",\n"
        "      \"enabled\": true,\n"
        "      \"config\": { \"max_capacity\": 10000, \"ttl_seconds\": 3600 }\n"
        "    },\n"
        "    {\n"
        "      \"name\": \"rag\",\n"
        "      \"enabled\": true,\n"
        "      \"config\": {\n"
        "        \"embedding_model\": \"nomic-embed-text-v2\",\n"
        "        \"routes\": [{\n"
        "          \"model_pattern\": \"rag-emails\",\n"
        "          \"collection_name\": \"emails\",\n"
        "          \"system_prompt_template\": \"Contexte : {context}\"\n"
        "        }]\n"
        "      }\n"
        "    }\n"
        "  ]\n"
        "}"
    )

    tb_sjson = s11.shapes.add_textbox(Inches(1.0), Inches(1.95), Inches(6.4), Inches(4.6))
    tf_sjson = tb_sjson.text_frame
    tf_sjson.word_wrap = True
    tf_sjson.margin_left = tf_sjson.margin_right = tf_sjson.margin_top = tf_sjson.margin_bottom = 0
    p = tf_sjson.paragraphs[0]
    p.text = sample_json
    p.font.name = FONT_CODE
    p.font.size = Pt(8.5)
    p.font.color.rgb = COLOR_CODE_TEXT

    # Right: Annotations & Best Practices
    add_card(s11, Inches(7.8), Inches(1.4), Inches(4.7), Inches(5.35), bg_color=COLOR_CARD_BG, border_color=COLOR_BORDER)
    add_badge(s11, Inches(8.1), Inches(1.65), Inches(2.8), Inches(0.35), "BONNES PRATIQUES & SÉCURITÉ", COLOR_BURGUNDY, font_size=10)

    tb_not = s11.shapes.add_textbox(Inches(8.1), Inches(2.15), Inches(4.1), Inches(4.4))
    tf_not = tb_not.text_frame
    tf_not.word_wrap = True

    notes = [
        ("Zéro Secret en Clair", "Utilisation de la syntaxe 'env.VARIABLE' permettant à Pylos de résoudre dynamiquement les clés d'API injectées par Vault ou Kubernetes Secrets."),
        ("Pondération des Modèles", "L'attribut 'weight' permet de répartir la charge entre plusieurs comptes ou abonnements LLM."),
        ("File d'Attente Concurrente", "Gestion automatique des pics de charge avec 'max_concurrency' et temporisation 'queue_timeout_ms' évitant la saturation des sockets."),
        ("Généralisation des Plugins", "Chaque plugin (PrefixCache, RAG, SemanticCache) s'active et se paramètre sans recompilation du code binaire."),
        ("Rechargement 'Live'", "Toute modification de ce fichier est prise en compte via POST /config/reload sans perte de connexion.")
    ]
    for i, (nh, nd) in enumerate(notes):
        p = tf_not.paragraphs[0] if i == 0 else tf_not.add_paragraph()
        p.text = f"• {nh} :"
        p.font.name = FONT_TITLE
        p.font.size = Pt(10)
        p.font.bold = True
        p.font.color.rgb = COLOR_BURGUNDY
        p.space_before = Pt(3)

        p2 = tf_not.add_paragraph()
        p2.text = nd
        p2.font.name = FONT_BODY
        p2.font.size = Pt(9)
        p2.font.color.rgb = COLOR_TEXT_DARK

    # =========================================================================
    # SLIDE 12: Synthèse Exécutive & Matrice Décisionnelle
    # =========================================================================
    s12 = prs.slides.add_slide(blank_layout)
    apply_header(s12, "Synthèse Exécutive : Bilan, Valeur & Prochaines Étapes", "SYNTHÈSE DE PROJET", 12)

    # 4 Status summary cards horizontally
    cards_summary = [
        ("FAIT : INFERENCE & HA", "Opérationnel", COLOR_GREEN, [
            "20+ fournisseurs intégrés",
            "Streaming SSE natif",
            "Dynamic Batching complet",
            "Fallback & Circuit Breaker"
        ]),
        ("FAIT : CACHE & RAG", "Opérationnel", COLOR_GREEN, [
            "Cache TinyLFU en mémoire",
            "Cache sémantique Qdrant",
            "Graphe de mémoire Cypher",
            "Parent-Child & CRAG"
        ]),
        ("FAIT : CONTRÔLE & UI", "Opérationnel", COLOR_GREEN, [
            "Virtual Keys & Quotas $",
            "Proxy MCP avec ACL",
            "16 pages React 19 complètes",
            "Observabilité Prometheus"
        ]),
        ("RESTE : PRODUCTION READY", "À Conduire", COLOR_BURGUNDY, [
            "Campagne SIT de bout en bout",
            "Stress-tests p99 à 10k req/s",
            "Connecteur natif pgvector",
            "Certification de production"
        ])
    ]

    card_ws = Inches(2.75)
    card_hs = Inches(3.6)
    for i, (title, status, color, points) in enumerate(cards_summary):
        cx = Inches(0.8) + i * Inches(2.95)
        cy = Inches(1.5)
        c = add_card(s12, cx, cy, card_ws, card_hs, bg_color=COLOR_CARD_BG, border_color=color)

        # Header badge
        add_badge(s12, cx + Inches(0.15), cy + Inches(0.2), card_ws - Inches(0.3), Inches(0.35), title, color, font_size=9.5)

        # Status tag
        st_box = s12.shapes.add_textbox(cx + Inches(0.1), cy + Inches(0.6), card_ws - Inches(0.2), Inches(0.35))
        tf_st = st_box.text_frame
        p = tf_st.paragraphs[0]
        p.alignment = PP_ALIGN.CENTER
        p.text = f"Statut : {status.upper()}"
        p.font.name = FONT_TITLE
        p.font.size = Pt(9.5)
        p.font.bold = True
        p.font.color.rgb = color

        # Divider
        div = s12.shapes.add_shape(MSO_SHAPE.RECTANGLE, cx + Inches(0.2), cy + Inches(0.95), card_ws - Inches(0.4), Inches(0.01))
        div.fill.solid()
        div.fill.fore_color.rgb = COLOR_BORDER
        div.line.fill.background()

        # Points
        bb = s12.shapes.add_textbox(cx + Inches(0.15), cy + Inches(1.05), card_ws - Inches(0.3), Inches(2.4))
        tf_bb = bb.text_frame
        tf_bb.word_wrap = True
        for j, pt in enumerate(points):
            p = tf_bb.paragraphs[0] if j == 0 else tf_bb.add_paragraph()
            p.text = f"✔  {pt}" if color == COLOR_GREEN else f"➔  {pt}"
            p.font.name = FONT_BODY
            p.font.size = Pt(9.5)
            p.font.color.rgb = COLOR_TEXT_DARK
            p.space_before = Pt(3)

    # Bottom Decision / Recommendation Box
    dec_box = add_card(s12, Inches(0.8), Inches(5.3), Inches(11.7), Inches(1.45), bg_color=COLOR_BG_LIGHT, border_color=COLOR_NAVY)
    
    # Left accent bar
    bar = s12.shapes.add_shape(MSO_SHAPE.RECTANGLE, Inches(0.8), Inches(5.3), Inches(0.15), Inches(1.45))
    bar.fill.solid()
    bar.fill.fore_color.rgb = COLOR_NAVY
    bar.line.fill.background()

    tb_dec = s12.shapes.add_textbox(Inches(1.2), Inches(5.4), Inches(11.0), Inches(1.25))
    tf_dec = tb_dec.text_frame
    tf_dec.word_wrap = True
    p = tf_dec.paragraphs[0]
    p.text = "CONCLUSION & VERDICT TECHNIQUE"
    p.font.name = FONT_TITLE
    p.font.size = Pt(11)
    p.font.bold = True
    p.font.color.rgb = COLOR_NAVY

    p2 = tf_dec.add_paragraph()
    p2.text = (
        "Pylos dispose d'une architecture Rust de premier ordre, robuste, modulaire et extrêmement riche sur le plan fonctionnel (95/95 tests unitaires réussis, code source vérifié, dashboard React 100% complété). "
        "Le gateway offre un retour sur investissement immédiat en rationalisant l'accès aux LLMs et en réduisant drastiquement les coûts de tokens. "
        "L'effort prioritaire résiduel consiste à conduire la campagne de validation SIT (System Integration Testing) pour certifier formellement le passage en production."
    )
    p2.font.name = FONT_BODY
    p2.font.size = Pt(9.5)
    p2.font.color.rgb = COLOR_TEXT_DARK
    p2.space_before = Pt(3)

    prs.save(output_path)
    print(f"Presentation saved successfully to: {output_path}")

if __name__ == "__main__":
    out_dir = r"E:\git\JZacharie\Pylos"
    out_file = os.path.join(out_dir, "Pylos_Presentation_Complete.pptx")
    build_presentation(out_file)
