import { useState } from 'react'
import { BotCard, BotCardSkeleton } from './components/BotCard'
import { VerificationPanel, VerificationPanelSkeleton } from './components/VerificationPanel'
import { RegistrationForm } from './components/RegistrationForm'
import { LoginForm } from './components/LoginForm'
import { Dashboard } from './components/Dashboard'
import { PricingTable } from './components/PricingTable'
import { useBots } from './hooks/useBots'
import { useAttestation } from './hooks/useAttestation'
import { useAuth } from './hooks/useAuth'
import { motion, AnimatePresence } from 'framer-motion'
import { AlertCircle, RefreshCw, Shield, Lock, Cpu, Plus, X, LogIn } from 'lucide-react'

function App() {
  const { data: bots, isLoading: botsLoading, error: botsError, refetch: refetchBots } = useBots()
  const { isLoading: attestationLoading } = useAttestation()
  const auth = useAuth()
  const [showRegistration, setShowRegistration] = useState(false)
  const [showLogin, setShowLogin] = useState(false)

  return (
    <div className="min-h-screen w-full flex flex-col">
      {/* Floating particles */}
      <div className="particle" />
      <div className="particle" />
      <div className="particle" />

      {/* Hero Section */}
      <section className="w-full py-20 md:py-28">
        <div className="max-w-6xl mx-auto px-6 md:px-12 lg:px-20">
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5 }}
            className="flex flex-col items-center text-center"
          >
            <div className="inline-flex items-center gap-2 px-4 py-2 mb-8 rounded-full glass-card text-sm font-medium">
              <span className="w-2 h-2 bg-emerald-500 rounded-full animate-pulse" />
              <span className="text-[var(--text-secondary)]">Secured by Intel TDX</span>
            </div>

            <h1 className="text-5xl md:text-7xl font-bold mb-6 leading-tight">
              <span className="gradient-text">Private AI</span>
              <span className="text-[var(--text-primary)]"> Conversations</span>
            </h1>

            <p className="text-xl md:text-2xl text-[var(--text-secondary)] mb-10 max-w-2xl">
              Chat with AI through Signal, protected by hardware security.
            </p>

            <div className="flex flex-wrap justify-center gap-8 text-sm text-[var(--text-muted)]">
              <div className="flex items-center gap-2">
                <Shield className="w-5 h-5 text-[var(--accent-start)]" />
                <span>End-to-End Encrypted</span>
              </div>
              <div className="flex items-center gap-2">
                <Cpu className="w-5 h-5 text-[var(--accent-mid)]" />
                <span>TEE Protected</span>
              </div>
              <div className="flex items-center gap-2">
                <Lock className="w-5 h-5 text-[var(--accent-end)]" />
                <span>Verifiable</span>
              </div>
            </div>
          </motion.div>
        </div>
      </section>

      {/* Dashboard or Registration Section */}
      <section id="register" className="w-full pb-20 md:pb-28">
        <div className="max-w-2xl mx-auto px-6 md:px-12 lg:px-20">
          {auth.isAuthenticated && auth.phone ? (
            <Dashboard phone={auth.phone} onLogout={auth.logout} />
          ) : (
            <>
              <motion.div
                initial={{ opacity: 0, y: 20 }}
                whileInView={{ opacity: 1, y: 0 }}
                viewport={{ once: true }}
                transition={{ duration: 0.5 }}
                className="flex flex-col items-center text-center mb-8"
              >
                <h2 className="text-3xl md:text-4xl font-bold mb-4">
                  <span className="gradient-text">Create Your Bot</span>
                </h2>
                <p className="text-[var(--text-secondary)] text-lg mb-6">
                  Register a phone number and configure your personal AI assistant
                </p>

                {!showRegistration && !showLogin && (
                  <div className="flex gap-3">
                    <button
                      onClick={() => setShowRegistration(true)}
                      className="glass-button glass-button-primary flex items-center gap-2 px-6 py-3"
                    >
                      <Plus className="w-5 h-5" />
                      Register New Bot
                    </button>
                    <button
                      onClick={() => setShowLogin(true)}
                      className="glass-button flex items-center gap-2 px-6 py-3"
                    >
                      <LogIn className="w-5 h-5" />
                      Log In
                    </button>
                  </div>
                )}
              </motion.div>

              <AnimatePresence>
                {showLogin && (
                  <motion.div
                    initial={{ opacity: 0, height: 0 }}
                    animate={{ opacity: 1, height: 'auto' }}
                    exit={{ opacity: 0, height: 0 }}
                    className="overflow-hidden"
                  >
                    <div className="relative">
                      <button
                        onClick={() => setShowLogin(false)}
                        className="absolute -top-2 -right-2 z-10 w-8 h-8 rounded-full bg-white/10 hover:bg-white/20 flex items-center justify-center transition-colors"
                      >
                        <X className="w-4 h-4 text-[var(--text-muted)]" />
                      </button>
                      <LoginForm
                        onLogin={auth.login}
                        loading={auth.loading}
                        error={auth.error}
                      />
                    </div>
                  </motion.div>
                )}

                {showRegistration && (
                  <motion.div
                    initial={{ opacity: 0, height: 0 }}
                    animate={{ opacity: 1, height: 'auto' }}
                    exit={{ opacity: 0, height: 0 }}
                    className="overflow-hidden"
                  >
                    <div className="relative">
                      <button
                        onClick={() => setShowRegistration(false)}
                        className="absolute -top-2 -right-2 z-10 w-8 h-8 rounded-full bg-white/10 hover:bg-white/20 flex items-center justify-center transition-colors"
                      >
                        <X className="w-4 h-4 text-[var(--text-muted)]" />
                      </button>
                      <RegistrationForm
                        onComplete={() => {
                          setShowRegistration(false)
                          refetchBots()
                        }}
                      />
                    </div>
                  </motion.div>
                )}
              </AnimatePresence>
            </>
          )}
        </div>
      </section>

      {/* Pricing Section */}
      <section className="w-full pb-20 md:pb-28 border-t border-white/5 pt-20">
        <div className="max-w-3xl mx-auto px-6 md:px-12 lg:px-20">
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            transition={{ duration: 0.5 }}
            className="flex flex-col items-center text-center mb-12"
          >
            <h2 className="text-3xl md:text-4xl font-bold mb-4">
              <span className="gradient-text">Simple Pricing</span>
            </h2>
            <p className="text-[var(--text-secondary)] text-lg">
              Start free, upgrade when you need more
            </p>
          </motion.div>

          <PricingTable />
        </div>
      </section>

      {/* Bot Cards Section */}
      <section className="w-full pb-20 md:pb-28 border-t border-white/5 pt-20">
        <div className="max-w-2xl mx-auto px-6 md:px-12 lg:px-20">
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            transition={{ duration: 0.5 }}
            className="flex flex-col items-center text-center mb-12"
          >
            <h2 className="text-3xl md:text-4xl font-bold mb-4">
              <span className="gradient-text">Active Bots</span>
            </h2>
            <p className="text-[var(--text-secondary)] text-lg">
              Currently registered AI assistants
            </p>
          </motion.div>

          {botsError ? (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className="glass-card p-10 flex flex-col items-center text-center"
            >
              <div className="w-16 h-16 mb-4 rounded-full bg-amber-500/10 flex items-center justify-center">
                <AlertCircle className="w-8 h-8 text-amber-400" />
              </div>
              <h3 className="text-xl font-semibold text-[var(--text-primary)] mb-3">
                Could not load bots
              </h3>
              <p className="text-[var(--text-muted)] mb-6">
                The bot directory is temporarily unavailable.
              </p>
              <button
                onClick={() => refetchBots()}
                className="glass-button flex items-center gap-2"
              >
                <RefreshCw className="w-4 h-4" />
                Retry
              </button>
            </motion.div>
          ) : botsLoading ? (
            <BotCardSkeleton />
          ) : bots && bots.length > 0 ? (
            <div className="space-y-8">
              {bots.map((bot, index) => (
                <BotCard key={bot.username} bot={bot} index={index} featured={bots.length === 1} />
              ))}
            </div>
          ) : (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className="glass-card p-8 flex flex-col items-center text-center"
            >
              <p className="text-[var(--text-muted)]">No bots registered yet. Create one above!</p>
            </motion.div>
          )}
        </div>
      </section>

      {/* Verification Section */}
      <section id="verify" className="w-full py-20 md:py-28 border-t border-white/5">
        <div className="max-w-3xl mx-auto px-6 md:px-12 lg:px-20">
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            transition={{ duration: 0.5 }}
            className="flex flex-col items-center text-center mb-12"
          >
            <h2 className="text-3xl md:text-4xl font-bold mb-4">
              <span className="gradient-text">Verify Security</span>
            </h2>
            <p className="text-[var(--text-secondary)] text-lg">
              Don't trust us — verify the cryptographic proof yourself.
            </p>
          </motion.div>

          {attestationLoading ? (
            <VerificationPanelSkeleton />
          ) : (
            <VerificationPanel />
          )}
        </div>
      </section>

      {/* Footer */}
      <footer className="w-full py-10 border-t border-white/5 mt-auto">
        <div className="max-w-6xl mx-auto px-6 md:px-12 lg:px-20">
          <p className="text-[var(--text-muted)] text-sm text-center">
            Powered by{' '}
            <a href="https://near.ai" target="_blank" rel="noopener noreferrer" className="hover:text-[var(--text-secondary)] transition-colors">
              NEAR AI
            </a>
            {' · '}
            Secured by{' '}
            <a href="https://phala.network" target="_blank" rel="noopener noreferrer" className="hover:text-[var(--text-secondary)] transition-colors">
              Phala Network
            </a>
            {' · '}
            <a href="https://github.com/zmanian/signal-bot-tee" target="_blank" rel="noopener noreferrer" className="hover:text-[var(--text-secondary)] transition-colors">
              Open Source
            </a>
          </p>
        </div>
      </footer>
    </div>
  )
}

export default App
