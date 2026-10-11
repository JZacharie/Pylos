// Pylos Documentation — Scripts interactifs légers (sans framework)
(function () {
  "use strict";

  var racine = document.documentElement;

  // -------------------------------------------------- Thème Clair / Sombre
  var CLE_THEME = "pylos-theme";
  var boutonTheme = document.querySelector("[data-theme-toggle]");

  function estClair() {
    var choisi = racine.getAttribute("data-theme");
    if (choisi) return choisi === "light";
    return !!(window.matchMedia && window.matchMedia("(prefers-color-scheme: light)").matches);
  }

  function majBoutonTheme() {
    if (!boutonTheme) return;
    var clair = estClair();
    boutonTheme.textContent = clair ? "Thème sombre" : "Thème clair";
    boutonTheme.setAttribute("aria-pressed", clair ? "true" : "false");
  }

  if (boutonTheme) {
    majBoutonTheme();
    boutonTheme.addEventListener("click", function () {
      var suivant = estClair() ? "dark" : "light";
      racine.setAttribute("data-theme", suivant);
      try { localStorage.setItem(CLE_THEME, suivant); } catch (e) {}
      majBoutonTheme();
    });
    if (window.matchMedia) {
      var mediaTheme = window.matchMedia("(prefers-color-scheme: light)");
      var surChangement = function () { majBoutonTheme(); };
      if (mediaTheme.addEventListener) mediaTheme.addEventListener("change", surChangement);
      else if (mediaTheme.addListener) mediaTheme.addListener(surChangement);
    }
  }

  // -------------------------------------------------- Barre de progression
  var barre = document.querySelector("[data-progress]");
  if (barre) {
    var majBarre = function () {
      var hauteur = document.documentElement.scrollHeight - window.innerHeight;
      var ratio = hauteur > 0 ? Math.min(1, Math.max(0, window.scrollY / hauteur)) : 0;
      barre.style.transform = "scaleX(" + ratio.toFixed(4) + ")";
    };
    window.addEventListener("scroll", majBarre, { passive: true });
    window.addEventListener("resize", majBarre);
    majBarre();
  }

  // -------------------------------------------------- Bouton retour en haut
  var btnHaut = document.querySelector("[data-top]");
  if (btnHaut) {
    var majBtnHaut = function () {
      if (window.scrollY > 400) {
        btnHaut.hidden = false;
      } else {
        btnHaut.hidden = true;
      }
    };
    window.addEventListener("scroll", majBtnHaut, { passive: true });
    btnHaut.addEventListener("click", function () {
      window.scrollTo({ top: 0, behavior: "smooth" });
    });
  }

  // -------------------------------------------------- Sommaire dynamique (TOC)
  var zoneSommaire = document.querySelector("[data-toc]");
  var titres = [].slice.call(document.querySelectorAll("main h2, main h3")).filter(function (titre) {
    return !titre.closest(".card, .kpi-card, .footer, figure, pre, details");
  });

  if (zoneSommaire && titres.length > 1) {
    var liste = document.createElement("ol");
    liste.className = "toc";
    titres.forEach(function (titre, index) {
      if (!titre.id) titre.id = "section-" + (index + 1);
      var item = document.createElement("li");
      item.className = titre.tagName === "H3" ? "niveau-3" : "niveau-2";
      var lien = document.createElement("a");
      lien.href = "#" + titre.id;
      lien.textContent = titre.textContent;
      item.appendChild(lien);
      liste.appendChild(item);
    });
    zoneSommaire.appendChild(liste);
    zoneSommaire.hidden = false;
    zoneSommaire.open = window.innerWidth >= 980;

    var liens = [].slice.call(liste.querySelectorAll("a"));
    var marquer = function (id) {
      liens.forEach(function (lien) {
        var actif = lien.getAttribute("href") === "#" + id;
        lien.className = actif ? "actif" : "";
        if (actif) lien.setAttribute("aria-current", "true");
        else lien.removeAttribute("aria-current");
      });
    };

    if ("IntersectionObserver" in window) {
      var obs = new IntersectionObserver(function (entries) {
        entries.forEach(function (entry) {
          if (entry.isIntersecting) {
            marquer(entry.target.id);
          }
        });
      }, { rootMargin: "0px 0px -70% 0px" });
      titres.forEach(function (t) { obs.observe(t); });
    }
  }

  // -------------------------------------------------- Boutons de copie pour les blocs de code
  var blocsPre = document.querySelectorAll("pre");
  blocsPre.forEach(function (pre) {
    var btn = document.createElement("button");
    btn.type = "button";
    btn.className = "btn-copy";
    btn.textContent = "Copier";
    btn.setAttribute("aria-label", "Copier le code");
    pre.appendChild(btn);

    btn.addEventListener("click", function () {
      var code = pre.querySelector("code");
      var texte = code ? code.innerText : pre.innerText;
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(texte).then(function () {
          btn.textContent = "Copié !";
          btn.classList.add("copied");
          setTimeout(function () {
            btn.textContent = "Copier";
            btn.classList.remove("copied");
          }, 2000);
        });
      }
    });
  });
})();
