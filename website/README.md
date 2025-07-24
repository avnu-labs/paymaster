# Paymaster landing page

This website serves as the primary landing page for the Paymaster service.

## 🚀 Quick Start

```bash
# Install dependencies
yarn install

# Start development server
yarn dev

# Build for production
yarn build

# Lint & format
yarn format && yarn lint
```

## 🛠️ Tech Stack

- **Framework**: React 18.2 with Vite 4.4
- **Styling**: Tailwind CSS 3.3
- **Animations**: Framer Motion 10.16
- **Icons**: Lucide React
- **SEO**: React Helmet with comprehensive meta tags
- **Development**: ESLint 9, Prettier, Knip

## 📁 Project Structure

```
website/
├── public/              # Static assets
│   ├── avnu_paymaster.mp4    # Promotional video
│   └── og-image.png          # Open Graph image
├── src/
│   ├── App.jsx          # Main application component
│   ├── main.jsx         # Application entry point
│   ├── index.css        # Global styles and animations
│   └── config/
│       └── app-config.js     # Configuration (links, SEO, etc.)
├── package.json         # Dependencies and scripts
├── vite.config.js       # Vite configuration
├── tailwind.config.js   # Tailwind CSS configuration
├── postcss.config.js    # PostCSS configuration
└── eslint.config.js     # ESLint configuration
```

## 🔧 Configuration

### Links and URLs

All external links and URLs are configured in `src/config/app-config.js`:

- Documentation: https://docs.out-of-gas.xyz
- GitHub: https://github.com/avnu-labs/paymaster
- Telegram: https://t.me/avnu_fi

### SEO Optimization

- Complete Open Graph tags for social sharing
- Twitter Card meta tags
- Structured data for search engines
- Canonical URL configuration

#### Open Graph Image

- Location: `public/og-image.png`
- Dimensions: 1200x630px (recommended for social media)
- Used for link previews on social platforms

## 🔗 External Resources

- **Main Documentation**: https://docs.out-of-gas.xyz
- **GitHub Repository**: https://github.com/avnu-labs/paymaster
- **Telegram Community**: https://t.me/avnu_fi
- **Main Service**: Parent directory contains the Rust-based paymaster service

## 📄 License

Part of the Paymaster project. See the main repository for license information.
