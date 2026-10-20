# Contrôle qualité (lot 26)

Ce qui est vérifié, comment le rejouer et ce qui a été trouvé.

| Contrôle | Commande | Résultat (2026-10-19) |
|---|---|---|
| Vitesse, accessibilité, bonnes pratiques, SEO (Lighthouse 12) | `npx lighthouse <url> [--preset=desktop]` (installé hors du dépôt) | site : performance 96–100, accessibilité 100, bonnes pratiques 100, SEO 100 (accueil, page outil, liste des outils) ; Flow : accessibilité 100, bonnes pratiques 100, performance 97 sur ordinateur et 66–71 sur téléphone avec ralentissement simulé (le programme WebAssembly se charge puis s'initialise) ; SEO 54 volontaire : les hôtes d'applications sont `noindex` |
| Accessibilité fine (axe-core, WCAG 2.1 AA) | script `axe` sur Flow : vide, avec cartes et panneau IA, bibliothèque, modèles, historique, thèmes clair et sombre | **1 défaut sérieux trouvé et corrigé** : contraste 3,6:1 de la couleur d'accent avec le blanc (tous les boutons principaux) ; accent passé de `#0a84ff` à `#0066cc` (5,6:1) ; depuis : aucune violation |
| En-têtes de sécurité | `node scripts/audit-headers.mjs` | 9 hôtes sur 9 : HSTS, CSP (`default-src 'self'`, `object-src 'none'`, `base-uri`, `frame-ancestors`), `nosniff`, `Referrer-Policy: no-referrer`, `Permissions-Policy` ; seule exception connue et documentée : `unsafe-eval` sur les hôtes d'applications (Dioxus) |
| Audit visuel (lot 31) | `AXE_PATH=<axe.min.js> CHROME_PATH=<chrome> node tests/e2e/visual-audit.mjs [url]` sur le site construit (`astro preview`) | 21 pages × 375/768/1280/1920 px × clair/sombre : débordement horizontal, cibles < 44 px, texte < 14 px, erreurs console, violations axe WCAG 2.1 AA ; 0 défaut bloquant le 2026-10-20 |
| Téléphone | écran d'iPhone 13 émulé | pas de défilement horizontal, mise en page en colonne |
| Firefox et WebKit (Safari) | `.github/workflows/browsers.yml` (Playwright n'installe pas ces moteurs sur macOS 13) | voir PROGRESS.md |
| Suivi d'erreurs léger | `POST /v1/telemetry` | compteurs anonymes par jour, application et type ; lecture : `wrangler d1 execute DB --remote --env production --command "SELECT * FROM client_errors ORDER BY day DESC"` |

Essai mesuré puis abandonné : annoncer le fichier WebAssembly par une balise `preload` dans la page. Mesuré sur un téléphone simulé, le premier affichage devenait plus lent (4,5–4,9 s sans, 6,9–15,6 s avec) ; la modification n'a pas été conservée.
