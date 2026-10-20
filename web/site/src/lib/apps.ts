import { appNames, domains } from './data';

export const apps = [
  {
    id: 'seek', name: appNames.seek, available: true, launchUrl: `https://${domains.seek}`,
    title: { fr: 'Trouvez le bon outil, en quelques mots.', en: 'Find the right tool in a few words.' },
    status: { fr: 'Première version', en: 'First release' },
    description: {
      fr: 'Décrivez votre besoin en quelques mots. Seek retrouve les outils et leurs variantes en français ou en anglais, localement et sans compte. La même recherche reste disponible sur le site.',
      en: 'Describe what you need in a few words. Seek finds tools and variants in French or English, locally and without an account. The same search remains available on the site.',
    },
    features: {
      fr: ['Recherche locale, sans envoyer vos mots-clés.', 'Accents, préfixes, synonymes et petites fautes pris en charge.', 'Résultats qui ouvrent directement l’outil ou sa variante.'],
      en: ['Local search without sending your keywords.', 'Supports accents, prefixes, synonyms and small typos.', 'Results open the tool or its variant directly.'],
    },
    next: { fr: 'Première version Rust/Dioxus. La recherche par IA, le préremplissage intelligent restent à terminer. Version ordinateur en bêta, installable depuis Kortex Store.', en: 'First Rust/Dioxus version. AI search, intelligent parameter presets remain to be completed. Desktop version in beta, installable from Kortex Store.' },
  },
  {
    id: 'store', name: appNames.store, available: true, launchUrl: `https://${domains.store}`,
    title: { fr: 'Vos outils, aussi sur votre ordinateur.', en: 'Your tools, also on your computer.' },
    status: { fr: 'Bêta sur ordinateur', en: 'Desktop beta' },
    description: {
      fr: 'Explorez les 38 outils dans le Store Rust/Dioxus : recherche, univers, fiches et permissions. Sur ordinateur, il installe, met à jour et supprime les outils et les apps Kortex à partir d’un catalogue signé ; un compte Kortex gratuit est nécessaire pour installer.',
      en: 'Explore the 38 tools in the Rust/Dioxus Store: search, universes, details and permissions. On your computer, it installs, updates and removes Kortex tools and apps from a signed catalogue; a free Kortex account is required to install.',
    },
    features: {
      fr: ['Catalogue Rust de 38 outils avec fiches détaillées.', 'Recherche et filtres par univers.', 'Installation vérifiée (signature et empreinte) sur ordinateur.'],
      en: ['Rust catalogue with 38 detailed tool listings.', 'Search and universe filters.', 'Verified installation (signature and checksum) on your computer.'],
    },
    next: { fr: 'Bêta : l’installation est vérifiée sur macOS ; la recette complète sous Windows et Linux et la connexion au compte depuis le Store restent à terminer (connexion par la commande kortex login).', en: 'Beta: installation is verified on macOS; full Windows and Linux acceptance checks and signing in from the Store remain to be completed (sign in with the kortex login command).' },
  },
  {
    id: 'desk', name: appNames.desk, available: true, launchUrl: `https://${domains.desk}`,
    title: { fr: 'Un espace pour organiser vos outils.', en: 'Arrange your tools in one workspace.' },
    status: { fr: 'Première version', en: 'First release' },
    description: { fr: 'Ouvrez plusieurs outils dans un espace de travail organisé en fenêtres. Vos dispositions restent enregistrées sur cet appareil.', en: 'Open multiple tools in a workspace arranged in windows. Your layouts are saved on this device.' },
    features: { fr: ['Déplacement, redimensionnement et réduction des fenêtres.', 'Dispositions en moitiés, tiers, quarts et grille.', 'Espaces nommés et sauvegarde locale des dispositions.'], en: ['Move, resize and minimize windows.', 'Half, third, quarter and grid layouts.', 'Named spaces with locally saved layouts.'] },
    next: { fr: 'Première version Rust/Dioxus. Les fichiers et résultats ne sont pas restaurés après rechargement. Sauvegarde manuelle du compte disponible. Synchronisation automatique et liaisons entre outils restent à terminer. Version ordinateur en bêta, installable depuis Kortex Store.', en: 'First Rust/Dioxus version. Files and results are not restored after reload. Manual account backup is available. Automatic sync and tool connections remain to be completed. Desktop version in beta, installable from Kortex Store.' },
  },
  {
    id: 'flow', name: appNames.flow, available: true, launchUrl: `https://${domains.flow}`,
    title: { fr: 'Composez des traitements réutilisables.', en: 'Compose reusable processing workflows.' },
    status: { fr: 'Première version', en: 'First release' },
    description: { fr: 'Reliez des outils sur un canevas pour répéter une suite de traitements, avec un déroulement visible étape par étape. Vos fichiers restent sur votre appareil.', en: 'Connect tools on a canvas to repeat a sequence of steps, with visible step-by-step execution. Your files stay on your device.' },
    features: { fr: ['Canevas nodal validé par les contrats des outils.', 'Exécution locale avec réessais, annulation et historique.', 'Bibliothèque, modèles, déclencheurs et nœud IA avec consentement.'], en: ['Node canvas validated by tool contracts.', 'Local execution with retries, cancellation and history.', 'Library, templates, triggers and an AI node with consent.'] },
    next: { fr: 'Première version Rust/Dioxus. Sur le web, un workflow ne fonctionne que pendant que son onglet est ouvert. Le nœud IA nécessite un compte. Les connexions OAuth aux services restent à terminer. Version ordinateur en bêta, installable depuis Kortex Store.', en: 'First Rust/Dioxus version. On the web, a workflow runs only while its tab is open. The AI node requires an account. OAuth service connections remain to be completed. Desktop version in beta, installable from Kortex Store.' },
  },
  {
    id: 'mind', name: appNames.mind, available: true, launchUrl: `https://${domains.mind}`,
    title: { fr: 'Un assistant qui prépare un plan visible.', en: 'An assistant that prepares a visible plan.' },
    status: { fr: 'Première version', en: 'First release' },
    description: { fr: 'Décrivez votre demande : Mind propose un plan de traitement avec les outils Kortex, puis l’exécute localement après votre validation.', en: 'Describe your request: Mind proposes a processing plan with Kortex tools, then runs it locally after your approval.' },
    features: { fr: ['Plan consultable avant toute exécution.', 'Consentement avant tout envoi de contenu à l’IA.', 'Crédits partagés et consommation visible.'], en: ['Reviewable plans before execution.', 'Consent before sending any content to AI.', 'Shared credits with visible usage.'] },
    next: { fr: 'Première version Rust/Dioxus. Un compte Kortex est nécessaire pour l’IA, avec des crédits limités. La synchronisation de l’historique reste à terminer. Version ordinateur en bêta, installable depuis Kortex Store.', en: 'First Rust/Dioxus version. A Kortex account is required for AI, with limited credits. History sync remains to be completed. Desktop version in beta, installable from Kortex Store.' },
  },
  {
    id: 'hub', name: appNames.hub, available: true, launchUrl: `https://${domains.hub}`,
    title: { fr: 'Votre tableau de bord Kortex.', en: 'Your Kortex dashboard.' },
    status: { fr: 'Première version', en: 'First release' },
    description: { fr: 'Retrouvez vos favoris, vos outils récents et votre activité locale dans un tableau de bord personnalisable.', en: 'Find your favourites, recent tools and local activity in a customizable dashboard.' },
    features: { fr: ['Widgets réorganisables, masquables et largeur réglable.', 'Favoris et ouvertures récentes enregistrés localement.', 'Recherche dans les 38 outils du catalogue web.'], en: ['Reorder, hide and resize dashboard widgets.', 'Locally saved favourites and recent launches.', 'Search the 38 tools in the web catalogue.'] },
    next: { fr: 'Première version Rust/Dioxus. Le compte reste accessible sur le site. Session du site reconnue et sauvegarde manuelle des métadonnées disponibles. Connexion native et synchronisation automatique restent à terminer. Version ordinateur en bêta, installable depuis Kortex Store.', en: 'First Rust/Dioxus version. Your account remains available on the site. Website session recognition and manual metadata backup are available. Native sign-in and automatic sync remain to be completed. Desktop version in beta, installable from Kortex Store.' },
  },
] as const;
export type AppInfo = typeof apps[number];
