export const appConfig = {
  // Application Info
  description:
    'Sponsor gas fees, accept any token, and control every detail of the gas experience. Empower your application with a SNIP‑29 compliant Paymaster.',

  // Links
  links: {
    documentation: 'https://docs.out-of-gas.xyz',
    github: 'https://github.com/avnu-labs/paymaster',
    telegram: 'https://t.me/avnu_developers',
  },

  // Copyright
  copyright: {
    name: 'avnu labs',
    year: new Date().getFullYear(),
    text: 'All rights reserved.',
  },

  // SEO & Social Media
  seo: {
    // Basic SEO
    title: 'Paymaster - Abstract Gas Fees on Starknet',
    titleTemplate: '%s | Paymaster by avnu Labs',
    defaultTitle: 'Paymaster - Abstract Gas Fees on Starknet',
    url: 'https://out-of-gas.xyz',
    siteName: 'Paymaster',
    locale: 'en_US',
    keywords:
      'starknet, paymaster, gas fees, blockchain, defi, web3, avnu, cairo, ethereum, layer 2',

    // Open Graph
    og: {
      type: 'website',
      image: 'https://out-of-gas.xyz/og-image.png', // Recommended: 1200x630px
      imageAlt: 'Paymaster - Abstract Gas Fees on Starknet',
      imageWidth: 1200,
      imageHeight: 630,
    },

    // Twitter
    twitter: {
      card: 'summary_large_image',
      site: '@avnu_fi',
      creator: '@avnu_fi',
      image: '/og-image.png', // Can be same as OG image
    },

    // Additional Meta
    themeColor: '#3761F6',
    author: 'avnu Labs',
    robots: 'index, follow',
    canonical: 'https://out-of-gas.xyz', // TODO: Update with actual URL
  },
};
