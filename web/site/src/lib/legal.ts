import type { Lang } from './data';

export interface LegalPage { slug: Record<Lang, string>; title: Record<Lang, string>; body: Record<Lang, string[]>; draft?: boolean }

// TODO(launch): mentions légales need the publisher's identity (LCEN art. 6). Fields are marked "[À COMPLÉTER]".
export const legalPages: LegalPage[] = [
  {
    slug: { fr: 'confidentialite', en: 'privacy' },
    title: { fr: 'Politique de confidentialité', en: 'Privacy policy' },
    body: {
      fr: [
        '## L\'essentiel',
        'Les outils Kortex fonctionnent sur votre appareil. Les fichiers et textes que vous traitez ne sont jamais envoyés à nos serveurs.',
        '## Données collectées',
        'Ce site n\'utilise aucun cookie de suivi et aucun outil publicitaire. Nous ne collectons pas le contenu de vos fichiers. Les polices et ressources sont hébergées par nous, sans appel à des services tiers.',
        'Notre hébergeur technique, Cloudflare, traite des journaux techniques (adresse IP, date, page demandée) pour diffuser le site et assurer sa sécurité.',
        '## Stockage local',
        'Vos préférences (thème, mode) peuvent être conservées dans le stockage local de votre navigateur. Elles ne quittent pas votre appareil.',
        '## Vos droits',
        'Conformément au RGPD, vous pouvez demander l\'accès, la rectification ou l\'effacement des données vous concernant. Contact : [À COMPLÉTER].',
      ],
      en: [
        '## In short',
        'Kortex tools run on your device. The files and text you process are never sent to our servers.',
        '## Data we collect',
        'This site uses no tracking cookies and no advertising tools. We do not collect the content of your files. Fonts and assets are self-hosted, with no calls to third-party services.',
        'Our technical host, Cloudflare, processes technical logs (IP address, date, requested page) to deliver the site and keep it secure.',
        '## Local storage',
        'Your preferences (theme, mode) may be kept in your browser\'s local storage. They never leave your device.',
        '## Your rights',
        'Under the GDPR you may request access to, correction of, or deletion of your personal data. Contact: [TO COMPLETE].',
      ],
    },
  },
  {
    slug: { fr: 'conditions', en: 'terms' },
    title: { fr: 'Conditions d\'utilisation', en: 'Terms of use' },
    body: {
      fr: [
        '## Utilisation',
        'Kortex est fourni gratuitement, « en l\'état », sans garantie de disponibilité. Vous restez responsable de l\'usage que vous faites des outils et des fichiers que vous traitez.',
        '## Propriété',
        'Le code et la marque Kortex sont protégés ; tous droits réservés. Vos fichiers vous appartiennent et restent sur votre appareil.',
        '## Limites',
        'Les résultats des outils (conversions, calculs) sont donnés à titre indicatif ; vérifiez-les avant tout usage important. Nous ne pouvons être tenus responsables d\'une perte de données ou d\'un résultat inexact.',
      ],
      en: [
        '## Use',
        'Kortex is provided free of charge, "as is", with no availability guarantee. You remain responsible for how you use the tools and the files you process.',
        '## Ownership',
        'The Kortex code and brand are protected; all rights reserved. Your files belong to you and stay on your device.',
        '## Limits',
        'Tool results (conversions, calculations) are indicative; check them before any important use. We cannot be held liable for data loss or inaccurate results.',
      ],
    },
  },
  {
    slug: { fr: 'mentions-legales', en: 'legal-notice' },
    title: { fr: 'Mentions légales', en: 'Legal notice' },
    draft: true,
    body: {
      fr: [
        '## Éditeur',
        'Éditeur : Tiago Antunes, personne physique, développeur indépendant.',
        'Région : Occitanie, France.',
        'Adresse postale : [À COMPLÉTER]',
        'E-mail de contact : [À COMPLÉTER]',
        'Directeur de la publication : Tiago Antunes.',
        '## Hébergeur',
        'Cloudflare, Inc., 101 Townsend St, San Francisco, CA 94107, États-Unis.',
      ],
      en: [
        '## Publisher',
        'Publisher: Tiago Antunes, individual, independent developer.',
        'Region: Occitanie, France.',
        'Postal address: [TO COMPLETE]',
        'Contact e-mail: [TO COMPLETE]',
        'Publication director: Tiago Antunes.',
        '## Host',
        'Cloudflare, Inc., 101 Townsend St, San Francisco, CA 94107, USA.',
      ],
    },
  },
];
