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
        '## Compte facultatif',
        'Si vous créez un compte, nous conservons votre e-mail, un vérificateur dérivé de votre mot de passe, vos réglages et les métadonnées des appareils et installations jusqu’à la suppression du compte. Le mot de passe en clair et les fichiers traités ne sont pas transmis.',
        'Un cookie de session strictement nécessaire, HttpOnly et Secure, est utilisé uniquement sur api.kortex.site, pour une durée maximale de 30 jours. Les sessions expirées sont purgées quotidiennement. Aucun cookie de suivi n’est ajouté.',
        'Les clés anti-abus sont hachées et conservées au maximum 48 heures ; les statistiques d’utilisation agrégées sont conservées environ 90 jours. Vous pouvez exporter vos données et supprimer votre compte depuis la page Compte. Les sauvegardes de restauration de la base expirent sous sept jours.',
        'Si vous ajoutez une passkey, nous conservons sa clé publique, son nom et ses dates de création et d’utilisation. La clé privée et les données biométriques restent gérées par votre appareil. Les demandes de connexion expirent après cinq minutes et sont purgées quotidiennement. Vous pouvez retirer vos passkeys, fermer vos autres sessions et changer votre mot de passe depuis le compte.',
        'L’IA et les applications de synchronisation ne sont pas encore ouvertes au public.',
        '## Stockage local',
        'Vos préférences (thème, mode) peuvent être conservées dans le stockage local de votre navigateur. Elles ne quittent pas votre appareil.',
        '## Vos droits',
        'Conformément au RGPD, vous pouvez demander l\'accès, la rectification ou l\'effacement des données vous concernant. Contact : admin@kortex.site.',
      ],
      en: [
        '## In short',
        'Kortex tools run on your device. The files and text you process are never sent to our servers.',
        '## Data we collect',
        'This site uses no tracking cookies and no advertising tools. We do not collect the content of your files. Fonts and assets are self-hosted, with no calls to third-party services.',
        'Our technical host, Cloudflare, processes technical logs (IP address, date, requested page) to deliver the site and keep it secure.',
        '## Optional account',
        'If you create an account, we store your email, a verifier derived from your password, settings, and device and installation metadata until account deletion. Your plaintext password and processed files are not transmitted.',
        'A strictly necessary HttpOnly, Secure session cookie is used only on api.kortex.site for up to 30 days. Expired sessions are removed daily. No tracking cookie is added.',
        'Anti-abuse keys are hashed and retained for up to 48 hours; aggregate usage statistics are retained for approximately 90 days. You can export your data and delete your account from the Account page. Database recovery backups expire within seven days.',
        'If you add a passkey, we store its public key, name and creation/use dates. Your device manages the private key and biometric data. Sign-in challenges expire after five minutes and are purged daily. You can remove passkeys, close other sessions and change your password from your account.',
        'AI and synchronization apps are not yet open to the public.',
        '## Local storage',
        'Your preferences (theme, mode) may be kept in your browser\'s local storage. They never leave your device.',
        '## Your rights',
        'Under the GDPR you may request access to, correction of, or deletion of your personal data. Contact: admin@kortex.site.',
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
        'E-mail de contact : admin@kortex.site',
        'Directeur de la publication : Tiago Antunes.',
        '## Hébergeur',
        'Cloudflare, Inc., 101 Townsend St, San Francisco, CA 94107, États-Unis.',
      ],
      en: [
        '## Publisher',
        'Publisher: Tiago Antunes, individual, independent developer.',
        'Region: Occitanie, France.',
        'Postal address: [TO COMPLETE]',
        'Contact e-mail: admin@kortex.site',
        'Publication director: Tiago Antunes.',
        '## Host',
        'Cloudflare, Inc., 101 Townsend St, San Francisco, CA 94107, USA.',
      ],
    },
  },
];
