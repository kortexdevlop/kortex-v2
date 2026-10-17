import { appNames, domains } from './data';

export const apps = [
  {
    id: 'seek', name: appNames.seek, available: false,
    title: { fr: 'Trouvez le bon outil, en quelques mots.', en: 'Find the right tool in a few words.' },
    status: { fr: 'Recherche du site — application Rust à réaliser', en: 'Site search — Rust app pending' },
    description: {
      fr: 'Décrivez votre besoin dans la recherche du site. Le moteur local retrouve les outils et leurs variantes en français ou en anglais, sans compte.',
      en: 'Describe what you need in site search. The local engine finds tools and variants in French or English without an account.',
    },
    features: {
      fr: ['Recherche locale, sans envoyer vos mots-clés.', 'Accents, préfixes, synonymes et petites fautes pris en charge.', 'Ouverture avec ⌘K sur Mac ou Ctrl+K sur Windows et Linux.'],
      en: ['Local search without sending your keywords.', 'Supports accents, prefixes, synonyms and small typos.', 'Open with ⌘K on Mac or Ctrl+K on Windows and Linux.'],
    },
    next: { fr: 'La recherche par IA et le préremplissage intelligent sont prévus pour une prochaine étape.', en: 'AI search and intelligent parameter presets are planned for a later stage.' },
  },
  {
    id: 'store', name: appNames.store, available: true, launchUrl: `https://${domains.store}`,
    title: { fr: 'Vos outils, aussi sur votre ordinateur.', en: 'Your tools, also on your computer.' },
    status: { fr: 'Catalogue disponible', en: 'Catalogue available' },
    description: {
      fr: 'Explorez les 38 outils dans le Store Rust/Dioxus : recherche, univers, fiches et permissions. Les versions web sont accessibles immédiatement ; la distribution bureau reste en préparation.',
      en: 'Explore the 38 tools in the Rust/Dioxus Store: search, universes, details and permissions. Web tools are available immediately; desktop distribution is still being prepared.',
    },
    features: {
      fr: ['Catalogue Rust de 38 outils avec fiches détaillées.', 'Recherche et filtres par univers.', 'Ouverture immédiate des outils sur le web.'],
      en: ['Rust catalogue with 38 detailed tool listings.', 'Search and universe filters.', 'Open tools immediately on the web.'],
    },
    next: { fr: 'Les essais de distribution ont été réalisés localement. La publication des paquets et la recette complète sur les trois systèmes restent à terminer.', en: 'Distribution has been tested locally. Package publication and the full three-system acceptance checks remain to be completed.' },
  },
  {
    id: 'desk', name: appNames.desk, available: true, launchUrl: `https://${domains.desk}`,
    title: { fr: 'Un espace pour organiser vos outils.', en: 'Arrange your tools in one workspace.' },
    status: { fr: 'Première version', en: 'First release' },
    description: { fr: 'Ouvrez plusieurs outils dans un espace de travail organisé en fenêtres. Vos dispositions restent enregistrées sur cet appareil.', en: 'Open multiple tools in a workspace arranged in windows. Your layouts are saved on this device.' },
    features: { fr: ['Déplacement, redimensionnement et réduction des fenêtres.', 'Dispositions en moitiés, tiers, quarts et grille.', 'Espaces nommés et sauvegarde locale des dispositions.'], en: ['Move, resize and minimize windows.', 'Half, third, quarter and grid layouts.', 'Named spaces with locally saved layouts.'] },
    next: { fr: 'Première version Rust/Dioxus. Les fichiers et résultats ne sont pas restaurés après rechargement. Sauvegarde manuelle du compte disponible. Synchronisation automatique, liaisons entre outils et distribution bureau restent à terminer.', en: 'First Rust/Dioxus version. Files and results are not restored after reload. Manual account backup is available. Automatic sync, tool connections and desktop distribution remain to be completed.' },
  },
  {
    id: 'flow', name: appNames.flow, available: false,
    title: { fr: 'Composez des traitements réutilisables.', en: 'Compose reusable processing workflows.' },
    status: { fr: 'En préparation', en: 'In preparation' },
    description: { fr: 'Flow permettra de relier des outils dans un graphe pour répéter une suite de traitements avec un déroulement visible.', en: 'Flow will connect tools in a graph so you can repeat a sequence of steps with visible execution.' },
    features: { fr: ['Workflows locaux et inspection de chaque étape.', 'Entrées et sorties définies par les contrats d’outils.', 'Connecteurs et permissions explicites.'], en: ['Local workflows with inspection of each step.', 'Inputs and outputs defined by tool contracts.', 'Connectors with explicit permissions.'] },
    next: { fr: 'L’éditeur et les connecteurs restent à développer. Sur le web, un workflow ne pourra fonctionner que pendant que son onglet est ouvert ; les tâches en arrière-plan nécessiteront la version bureau.', en: 'The editor and connectors still need to be built. Web workflows will run only while their tab is open; background tasks will require the desktop version.' },
  },
  {
    id: 'mind', name: appNames.mind, available: false,
    title: { fr: 'Un assistant qui prépare un plan visible.', en: 'An assistant that prepares a visible plan.' },
    status: { fr: 'En préparation', en: 'In preparation' },
    description: { fr: 'Mind proposera des plans de traitement à partir de votre demande, puis utilisera les moteurs locaux après votre validation.', en: 'Mind will propose processing plans from your request and use local engines after your approval.' },
    features: { fr: ['Plan consultable avant toute exécution.', 'Consentement avant tout envoi de contenu à l’IA.', 'Crédits partagés et consommation visible.'], en: ['Reviewable plans before execution.', 'Consent before sending any content to AI.', 'Shared credits with visible usage.'] },
    next: { fr: 'L’interface de conversation et l’orchestration restent à développer. La passerelle et les crédits ont uniquement été testés localement.', en: 'The conversation interface and orchestration still need to be built. The gateway and credits have only been tested locally.' },
  },
  {
    id: 'hub', name: appNames.hub, available: true, launchUrl: `https://${domains.hub}`,
    title: { fr: 'Votre tableau de bord Kortex.', en: 'Your Kortex dashboard.' },
    status: { fr: 'Première version', en: 'First release' },
    description: { fr: 'Retrouvez vos favoris, vos outils récents et votre activité locale dans un tableau de bord personnalisable.', en: 'Find your favourites, recent tools and local activity in a customizable dashboard.' },
    features: { fr: ['Widgets réorganisables, masquables et largeur réglable.', 'Favoris et ouvertures récentes enregistrés localement.', 'Recherche dans les 38 outils du catalogue web.'], en: ['Reorder, hide and resize dashboard widgets.', 'Locally saved favourites and recent launches.', 'Search the 38 tools in the web catalogue.'] },
    next: { fr: 'Première version Rust/Dioxus. Le compte reste accessible sur le site. Session du site reconnue et sauvegarde manuelle des métadonnées disponibles. Connexion native, synchronisation automatique et distribution bureau restent à terminer.', en: 'First Rust/Dioxus version. Your account remains available on the site. Website session recognition and manual metadata backup are available. Native sign-in, automatic sync and desktop distribution remain to be completed.' },
  },
] as const;
export type AppInfo = typeof apps[number];
