import { useState } from 'react'
import { motion } from 'framer-motion'
import { Phone, Key, Loader2, AlertCircle, LogIn } from 'lucide-react'

interface LoginFormProps {
  onLogin: (phone: string, secret: string) => Promise<void>
  loading: boolean
  error: string | null
}

export function LoginForm({ onLogin, loading, error }: LoginFormProps) {
  const [phone, setPhone] = useState('')
  const [secret, setSecret] = useState('')

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!phone || !secret) return
    try {
      await onLogin(phone, secret)
    } catch {
      // error handled by parent
    }
  }

  return (
    <motion.form
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      onSubmit={handleSubmit}
      className="glass-card p-8 space-y-5 max-w-md mx-auto"
    >
      <div className="text-center mb-4">
        <h3 className="text-xl font-semibold text-[var(--text-primary)] mb-1">
          Manage Your Bot
        </h3>
        <p className="text-sm text-[var(--text-muted)]">
          Log in with your phone number and ownership secret
        </p>
      </div>

      <div>
        <label className="block text-sm font-medium text-[var(--text-secondary)] mb-2">
          Phone Number
        </label>
        <div className="relative">
          <Phone className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 text-[var(--text-muted)]" />
          <input
            type="tel"
            value={phone}
            onChange={(e) => setPhone(e.target.value)}
            placeholder="+1 555 123 4567"
            className="glass-input pl-12"
            autoFocus
          />
        </div>
      </div>

      <div>
        <label className="block text-sm font-medium text-[var(--text-secondary)] mb-2">
          Ownership Secret
        </label>
        <div className="relative">
          <Key className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 text-[var(--text-muted)]" />
          <input
            type="password"
            value={secret}
            onChange={(e) => setSecret(e.target.value)}
            placeholder="Your secret passphrase"
            className="glass-input pl-12"
          />
        </div>
      </div>

      {error && (
        <div className="flex items-center gap-2 p-3 rounded-lg bg-red-500/10 text-red-400 text-sm">
          <AlertCircle className="w-4 h-4 flex-shrink-0" />
          {error}
        </div>
      )}

      <button
        type="submit"
        disabled={loading || !phone || !secret}
        className="glass-button glass-button-primary w-full py-3 flex items-center justify-center gap-2"
      >
        {loading ? (
          <Loader2 className="w-5 h-5 animate-spin" />
        ) : (
          <>
            <LogIn className="w-5 h-5" />
            Log In
          </>
        )}
      </button>
    </motion.form>
  )
}
