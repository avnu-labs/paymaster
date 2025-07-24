
import React from 'react';
import { Helmet } from 'react-helmet';
import { motion } from 'framer-motion';
import { Github, Send } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Toaster } from '@/components/ui/toaster';
import { useToast } from '@/components/ui/use-toast';

function App() {
  const { toast } = useToast();

  const handleDocAccess = () => {
    toast({
      title: "🚧 This feature isn't implemented yet—but don't worry! You can request it in your next prompt! 🚀",
      duration: 4000,
    });
  };

  const handleSocialClick = (platform) => {
    toast({
      title: `🚧 ${platform} link isn't implemented yet—but don't worry! You can request it in your next prompt! 🚀`,
      duration: 4000,
    });
  };

  return (
    <>
      <Helmet>
        <title>BlockchainGas - Out of Gas? We've Got You Covered</title>
        <meta name="description" content="Revolutionary blockchain service providing instant gas solutions for your transactions. Never run out of gas again with our cutting-edge technology." />
      </Helmet>
      
      <div className="min-h-screen w-full flex flex-col items-center justify-center relative overflow-hidden">
        {/* Animated Background Elements */}
        <div className="absolute inset-0 overflow-hidden">
          <div className="absolute top-20 left-20 w-72 h-72 bg-blue-200/30 rounded-full blur-3xl floating-animation"></div>
          <div className="absolute bottom-20 right-20 w-96 h-96 bg-cyan-200/20 rounded-full blur-3xl floating-animation" style={{ animationDelay: '2s' }}></div>
          <div className="absolute top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2 w-80 h-80 bg-indigo-200/25 rounded-full blur-3xl floating-animation" style={{ animationDelay: '4s' }}></div>
        </div>

        {/* Main Content */}
        <main className="flex flex-col items-center justify-center text-center z-10 px-4 sm:px-6 lg:px-8">
          <motion.div
            initial={{ opacity: 0, y: 50 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 1, ease: "easeOut" }}
            className="space-y-8"
          >
            {/* Main Headline */}
            <motion.h1
              className="text-6xl sm:text-7xl md:text-8xl lg:text-9xl font-bold text-white drop-shadow-2xl tech-font"
              initial={{ opacity: 0, scale: 0.8 }}
              animate={{ opacity: 1, scale: 1 }}
              transition={{ duration: 1.2, delay: 0.2, ease: "easeOut" }}
            >
              Out of gas?
            </motion.h1>

            {/* CTA Button */}
            <motion.div
              initial={{ opacity: 0, y: 30 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.8, delay: 0.6 }}
              className="pt-4"
            >
              <Button
                onClick={handleDocAccess}
                size="lg"
                className="bg-gradient-to-r from-blue-500 to-cyan-500 hover:from-blue-600 hover:to-cyan-600 text-white font-semibold px-12 py-6 text-xl rounded-2xl pulse-glow transition-all duration-300 transform hover:scale-105 shadow-2xl border-0"
              >
                Access the doc
              </Button>
            </motion.div>
          </motion.div>
        </main>

        {/* Footer Overlay */}
        <motion.footer
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.8, delay: 1 }}
          className="absolute bottom-0 left-0 right-0 p-6 bg-gradient-to-t from-white/10 to-transparent backdrop-blur-sm"
        >
          <div className="flex flex-col sm:flex-row items-center justify-between max-w-7xl mx-auto">
            {/* Social Links */}
            <div className="flex items-center space-x-6 mb-4 sm:mb-0">
              <motion.button
                onClick={() => handleSocialClick('GitHub')}
                whileHover={{ scale: 1.1 }}
                whileTap={{ scale: 0.95 }}
                className="p-3 bg-white/20 backdrop-blur-sm rounded-full hover:bg-white/30 transition-all duration-300 shadow-lg"
              >
                <Github className="w-6 h-6 text-gray-700" />
              </motion.button>
              
              <motion.button
                onClick={() => handleSocialClick('Telegram')}
                whileHover={{ scale: 1.1 }}
                whileTap={{ scale: 0.95 }}
                className="p-3 bg-white/20 backdrop-blur-sm rounded-full hover:bg-white/30 transition-all duration-300 shadow-lg"
              >
                <Send className="w-6 h-6 text-gray-700" />
              </motion.button>
            </div>

            {/* Copyright */}
            <div className="text-gray-600 text-sm font-medium">
              <span>© 2024 BlockchainGas. All rights reserved.</span>
            </div>
          </div>
        </motion.footer>

        <Toaster />
      </div>
    </>
  );
}

export default App;
