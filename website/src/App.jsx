import { Helmet } from 'react-helmet';
import { motion } from 'framer-motion';
import { Github, Send } from 'lucide-react';
import { appConfig } from '@/config/app-config';

function App() {
  return (
    <>
      <Helmet>
        {/* Basic Meta Tags */}
        <title>{appConfig.seo.title}</title>
        <meta name="description" content={appConfig.description} />
        <meta name="keywords" content={appConfig.seo.keywords} />
        <meta name="author" content={appConfig.seo.author} />
        <meta name="robots" content={appConfig.seo.robots} />
        <link rel="canonical" href={appConfig.seo.canonical} />
        <meta name="theme-color" content={appConfig.seo.themeColor} />

        {/* Open Graph Meta Tags */}
        <meta property="og:title" content={appConfig.seo.title} />
        <meta property="og:description" content={appConfig.description} />
        <meta property="og:type" content={appConfig.seo.og.type} />
        <meta property="og:url" content={appConfig.seo.url} />
        <meta property="og:site_name" content={appConfig.seo.siteName} />
        <meta property="og:image" content={appConfig.seo.og.image} />
        <meta property="og:image:alt" content={appConfig.seo.og.imageAlt} />
        <meta property="og:image:width" content={appConfig.seo.og.imageWidth} />
        <meta property="og:image:height" content={appConfig.seo.og.imageHeight} />
        <meta property="og:locale" content={appConfig.seo.locale} />

        {/* Twitter Card Meta Tags */}
        <meta name="twitter:card" content={appConfig.seo.twitter.card} />
        <meta name="twitter:site" content={appConfig.seo.twitter.site} />
        <meta name="twitter:creator" content={appConfig.seo.twitter.creator} />
        <meta name="twitter:title" content={appConfig.seo.title} />
        <meta name="twitter:description" content={appConfig.description} />
        <meta name="twitter:image" content={appConfig.seo.twitter.image} />

        {/* Additional Meta Tags */}
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        <meta httpEquiv="Content-Type" content="text/html; charset=utf-8" />
        <meta httpEquiv="X-UA-Compatible" content="IE=edge" />
      </Helmet>

      <div className="min-h-screen w-full flex flex-col items-center justify-center relative overflow-hidden">
        {/* Animated Background Elements */}
        <div className="absolute inset-0 overflow-hidden">
          <div className="absolute top-20 left-20 w-72 h-72 bg-[#3761F6]/20 rounded-full blur-3xl floating-animation"></div>
          <div
            className="absolute bottom-20 right-20 w-96 h-96 bg-[#5B7FFF]/15 rounded-full blur-3xl floating-animation"
            style={{ animationDelay: '2s' }}
          ></div>
          <div
            className="absolute top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2 w-80 h-80 bg-[#7B9AFF]/20 rounded-full blur-3xl floating-animation"
            style={{ animationDelay: '4s' }}
          ></div>
        </div>

        {/* Main Content */}
        <main className="flex flex-col items-center justify-center text-center z-10 px-4 sm:px-6 lg:px-8">
          <motion.div
            initial={{ opacity: 0, y: 50 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 1, ease: 'easeOut' }}
            className="space-y-8"
          >
            {/* Main Headline */}
            <motion.h1
              className="text-6xl sm:text-7xl md:text-8xl lg:text-9xl font-bold text-white drop-shadow-2xl tech-font mb-12"
              initial={{ opacity: 0, scale: 0.8 }}
              animate={{ opacity: 1, scale: 1 }}
              transition={{ duration: 1.2, delay: 0.2, ease: 'easeOut' }}
            >
              Out of gas?
            </motion.h1>

            {/* Centered Video */}
            <motion.div
              initial={{ opacity: 0, scale: 0.9 }}
              animate={{ opacity: 1, scale: 1 }}
              transition={{ duration: 1, delay: 0.4 }}
              className="relative my-8 w-full max-w-3xl mx-auto"
            >
              <div className="relative rounded-2xl overflow-hidden shadow-2xl">
                <video
                  src="/avnu_paymaster.mp4"
                  autoPlay
                  loop
                  muted
                  playsInline
                  className="w-full h-full object-cover rounded-2xl"
                  style={{ boxShadow: '0 20px 50px rgba(0, 0, 0, 0.3)' }}
                >
                  Your browser does not support the video tag.
                </video>
                <div className="absolute inset-0 rounded-2xl pointer-events-none bg-gradient-to-t from-black/20 to-transparent"></div>
              </div>
            </motion.div>

            {/* CTA Button */}
            <motion.div
              initial={{ opacity: 0, y: 30 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.8, delay: 0.8 }}
              className="pt-4"
            >
              <a
                href={appConfig.links.documentation}
                target="_blank"
                rel="noopener noreferrer"
                className="inline-block bg-gradient-to-r from-[#3761F6] to-[#5B7FFF] hover:from-[#2E50D5] hover:to-[#4A6FEE] text-white font-semibold px-12 py-2 text-xl rounded-xl pulse-glow transition-all duration-300 transform hover:scale-105 shadow-2xl border-0"
              >
                Access the doc
              </a>
            </motion.div>
          </motion.div>
        </main>

        {/* Footer Overlay */}
        <motion.footer
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.8, delay: 1 }}
          className="absolute bottom-0 left-0 right-0 p-6 bg-gradient-to-t from-black/20 to-transparent backdrop-blur-sm"
        >
          <div className="flex flex-col sm:flex-row items-center justify-between max-w-7xl mx-auto">
            {/* Social Links */}
            <div className="flex items-center space-x-6 mb-4 sm:mb-0">
              <motion.a
                href={appConfig.links.github}
                target="_blank"
                rel="noopener noreferrer"
                whileHover={{ scale: 1.1 }}
                whileTap={{ scale: 0.95 }}
                className="text-gray-400 hover:text-white transition-colors duration-300"
              >
                <Github className="w-6 h-6" />
              </motion.a>

              <motion.a
                href={appConfig.links.telegram}
                target="_blank"
                rel="noopener noreferrer"
                whileHover={{ scale: 1.1 }}
                whileTap={{ scale: 0.95 }}
                className="text-gray-400 hover:text-white transition-colors duration-300"
              >
                <Send className="w-6 h-6" />
              </motion.a>
            </div>

            {/* Copyright */}
            <div className="text-gray-400 text-sm font-medium">
              <span>
                © {appConfig.copyright.year} {appConfig.copyright.name}. {appConfig.copyright.text}
              </span>
            </div>
          </div>
        </motion.footer>
      </div>
    </>
  );
}

export default App;
