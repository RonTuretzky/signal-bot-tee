import { useState } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { Phone, Key, Check, Loader2, AlertCircle, ExternalLink, Cpu, MessageSquare, User } from 'lucide-react'
import { registerNumber, verifyRegistration, setUsername, updateProfile, AVAILABLE_MODELS } from '../lib/api'

type Step = 'phone' | 'captcha' | 'verify' | 'configure' | 'complete'

interface FormData {
  phoneNumber: string
  captcha: string
  useVoice: boolean
  ownershipSecret: string
  verificationCode: string
  model: string
  systemPrompt: string
  username: string
  displayName: string
}

const DEFAULT_SYSTEM_PROMPT = `You are a helpful AI assistant running securely in a Trusted Execution Environment (TEE). You're accessible via Signal for private, encrypted conversations. Be concise, helpful, and friendly.`

export function RegistrationForm({ onComplete }: { onComplete?: () => void }) {
  const [step, setStep] = useState<Step>('phone')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [formData, setFormData] = useState<FormData>({
    phoneNumber: '',
    captcha: '',
    useVoice: false,
    ownershipSecret: '',
    verificationCode: '',
    model: AVAILABLE_MODELS[0].id,
    systemPrompt: DEFAULT_SYSTEM_PROMPT,
    username: '',
    displayName: '',
  })

  const updateField = <K extends keyof FormData>(field: K, value: FormData[K]) => {
    setFormData(prev => ({ ...prev, [field]: value }))
    setError(null)
  }

  const handleRegister = async () => {
    if (!formData.phoneNumber) {
      setError('Please enter a phone number')
      return
    }
    // Basic E.164 validation
    const cleaned = formData.phoneNumber.replace(/[\s\-()]/g, '')
    if (!cleaned.match(/^\+[1-9]\d{6,14}$/)) {
      setError('Please enter a valid phone number in international format (e.g., +15551234567)')
      return
    }

    setLoading(true)
    setError(null)

    try {
      await registerNumber(formData.phoneNumber, {
        captcha: formData.captcha || undefined,
        use_voice: formData.useVoice,
        ownership_secret: formData.ownershipSecret || undefined,
      })
      setStep('verify')
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Registration failed')
    } finally {
      setLoading(false)
    }
  }

  const handleVerify = async () => {
    if (!formData.verificationCode) {
      setError('Please enter the verification code')
      return
    }

    setLoading(true)
    setError(null)

    try {
      await verifyRegistration(formData.phoneNumber, formData.verificationCode, {
        ownership_secret: formData.ownershipSecret || undefined,
      })
      setStep('configure')
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Verification failed')
    } finally {
      setLoading(false)
    }
  }

  const handleConfigure = async () => {
    setLoading(true)
    setError(null)

    try {
      // Set profile if display name provided
      if (formData.displayName) {
        await updateProfile(
          formData.phoneNumber,
          formData.displayName,
          `AI Assistant | Model: ${formData.model.split('/').pop()}`,
          formData.ownershipSecret || undefined
        )
      }

      // Set username if provided
      if (formData.username) {
        await setUsername(
          formData.phoneNumber,
          formData.username,
          formData.ownershipSecret || undefined
        )
      }

      setStep('complete')
      onComplete?.()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Configuration failed')
    } finally {
      setLoading(false)
    }
  }

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className="glass-card p-8 md:p-10"
    >
      {/* Progress indicator */}
      <div className="flex items-center justify-center gap-2 mb-8">
        {(['phone', 'verify', 'configure', 'complete'] as const).map((s, i) => (
          <div key={s} className="flex items-center">
            <div
              className={`w-8 h-8 rounded-full flex items-center justify-center text-sm font-medium transition-colors ${
                step === s
                  ? 'bg-gradient-to-r from-[var(--accent-start)] to-[var(--accent-end)] text-white'
                  : ['phone', 'verify', 'configure', 'complete'].indexOf(step) > i
                  ? 'bg-emerald-500/20 text-emerald-400'
                  : 'bg-white/5 text-[var(--text-muted)]'
              }`}
            >
              {['phone', 'verify', 'configure', 'complete'].indexOf(step) > i ? (
                <Check className="w-4 h-4" />
              ) : (
                i + 1
              )}
            </div>
            {i < 3 && (
              <div
                className={`w-12 h-0.5 mx-1 ${
                  ['phone', 'verify', 'configure', 'complete'].indexOf(step) > i
                    ? 'bg-emerald-500/50'
                    : 'bg-white/10'
                }`}
              />
            )}
          </div>
        ))}
      </div>

      <AnimatePresence mode="wait">
        {/* Step 1: Phone Number */}
        {step === 'phone' && (
          <motion.div
            key="phone"
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -20 }}
            className="space-y-6"
          >
            <div className="text-center mb-6">
              <h3 className="text-xl font-semibold text-[var(--text-primary)] mb-2">
                Register Your Bot
              </h3>
              <p className="text-[var(--text-muted)]">
                Enter the phone number you want to use for your AI bot
              </p>
            </div>

            <div className="space-y-4">
              <div>
                <label className="block text-sm font-medium text-[var(--text-secondary)] mb-2">
                  Phone Number
                </label>
                <div className="relative">
                  <Phone className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 text-[var(--text-muted)]" />
                  <input
                    type="tel"
                    value={formData.phoneNumber}
                    onChange={(e) => updateField('phoneNumber', e.target.value)}
                    placeholder="+1 555 123 4567"
                    className="glass-input pl-12"
                    autoFocus
                  />
                </div>
                <p className="text-xs text-[var(--text-muted)] mt-2">
                  Use international format (e.g., +1 for US)
                </p>
              </div>

              <div>
                <label className="block text-sm font-medium text-[var(--text-secondary)] mb-2">
                  Captcha Token (usually required)
                </label>
                <div className="relative">
                  <Key className="absolute left-4 top-3 w-5 h-5 text-[var(--text-muted)]" />
                  <textarea
                    value={formData.captcha}
                    onChange={(e) => updateField('captcha', e.target.value)}
                    placeholder="signalcaptcha://..."
                    rows={3}
                    className="glass-input pl-12 resize-none"
                  />
                </div>
                <a
                  href="https://signalcaptchas.org/registration/generate.html"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-1 text-xs text-[var(--accent-start)] hover:text-[var(--accent-end)] mt-2 transition-colors"
                >
                  Get captcha token
                  <ExternalLink className="w-3 h-3" />
                </a>
              </div>

              <div>
                <label className="block text-sm font-medium text-[var(--text-secondary)] mb-2">
                  Ownership Secret (optional, for account recovery)
                </label>
                <input
                  type="password"
                  value={formData.ownershipSecret}
                  onChange={(e) => updateField('ownershipSecret', e.target.value)}
                  placeholder="Enter a secret passphrase"
                  className="glass-input"
                />
              </div>

              <label className="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  checked={formData.useVoice}
                  onChange={(e) => updateField('useVoice', e.target.checked)}
                  className="w-4 h-4 rounded border-white/20 bg-white/5"
                />
                <span className="text-sm text-[var(--text-secondary)]">
                  Use voice call instead of SMS
                </span>
              </label>
            </div>

            {error && (
              <div className="space-y-1">
                <div className="flex items-center gap-2 p-3 rounded-lg bg-red-500/10 text-red-400 text-sm">
                  <AlertCircle className="w-4 h-4 flex-shrink-0" />
                  {error}
                </div>
                {error.toLowerCase().includes('captcha') && (
                  <p className="text-xs text-[var(--text-muted)] px-1">
                    Captcha tokens expire quickly. Get a fresh one and try again immediately.
                  </p>
                )}
                {error.toLowerCase().includes('rate') && (
                  <p className="text-xs text-[var(--text-muted)] px-1">
                    Signal rate-limits registration attempts. Wait 24 hours before trying again.
                  </p>
                )}
                {error.toLowerCase().includes('already registered') && (
                  <p className="text-xs text-[var(--text-muted)] px-1">
                    This number has stale data. The system will attempt to clean it up — try again.
                  </p>
                )}
              </div>
            )}

            <button
              onClick={handleRegister}
              disabled={loading || !formData.phoneNumber}
              className="glass-button glass-button-primary w-full py-3 flex items-center justify-center gap-2"
            >
              {loading ? (
                <Loader2 className="w-5 h-5 animate-spin" />
              ) : (
                <>
                  <Phone className="w-5 h-5" />
                  Send Verification Code
                </>
              )}
            </button>
          </motion.div>
        )}

        {/* Step 2: Verification Code */}
        {step === 'verify' && (
          <motion.div
            key="verify"
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -20 }}
            className="space-y-6"
          >
            <div className="text-center mb-6">
              <h3 className="text-xl font-semibold text-[var(--text-primary)] mb-2">
                Enter Verification Code
              </h3>
              <p className="text-[var(--text-muted)]">
                A code was sent to {formData.phoneNumber}
              </p>
            </div>

            <div>
              <label className="block text-sm font-medium text-[var(--text-secondary)] mb-2">
                6-Digit Code
              </label>
              <input
                type="text"
                value={formData.verificationCode}
                onChange={(e) => updateField('verificationCode', e.target.value.replace(/\D/g, '').slice(0, 6))}
                placeholder="123456"
                className="glass-input text-center text-2xl tracking-widest"
                maxLength={6}
                autoFocus
              />
            </div>

            <button
              onClick={handleRegister}
              disabled={loading}
              className="text-sm text-[var(--accent-start)] hover:text-[var(--accent-end)] transition-colors disabled:opacity-50"
            >
              {loading ? 'Sending...' : "Didn't receive a code? Resend"}
            </button>

            {error && (
              <div className="flex items-center gap-2 p-3 rounded-lg bg-red-500/10 text-red-400 text-sm">
                <AlertCircle className="w-4 h-4 flex-shrink-0" />
                {error}
              </div>
            )}

            <div className="flex gap-3">
              <button
                onClick={() => setStep('phone')}
                className="glass-button flex-1 py-3"
              >
                Back
              </button>
              <button
                onClick={handleVerify}
                disabled={loading || formData.verificationCode.length !== 6}
                className="glass-button glass-button-primary flex-1 py-3 flex items-center justify-center gap-2"
              >
                {loading ? (
                  <Loader2 className="w-5 h-5 animate-spin" />
                ) : (
                  <>
                    <Check className="w-5 h-5" />
                    Verify
                  </>
                )}
              </button>
            </div>
          </motion.div>
        )}

        {/* Step 3: Configure Bot */}
        {step === 'configure' && (
          <motion.div
            key="configure"
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -20 }}
            className="space-y-6"
          >
            <div className="text-center mb-6">
              <div className="w-16 h-16 rounded-full bg-emerald-500/20 flex items-center justify-center mx-auto mb-4">
                <Check className="w-8 h-8 text-emerald-400" />
              </div>
              <h3 className="text-xl font-semibold text-[var(--text-primary)] mb-2">
                Phone Verified!
              </h3>
              <p className="text-[var(--text-muted)]">
                Now configure your AI bot
              </p>
            </div>

            <div className="space-y-4">
              <div>
                <label className="block text-sm font-medium text-[var(--text-secondary)] mb-2">
                  <div className="flex items-center gap-2">
                    <User className="w-4 h-4" />
                    Display Name
                  </div>
                </label>
                <input
                  type="text"
                  value={formData.displayName}
                  onChange={(e) => updateField('displayName', e.target.value)}
                  placeholder="AI Assistant"
                  className="glass-input"
                />
              </div>

              <div>
                <label className="block text-sm font-medium text-[var(--text-secondary)] mb-2">
                  <div className="flex items-center gap-2">
                    <User className="w-4 h-4" />
                    Username (optional)
                  </div>
                </label>
                <input
                  type="text"
                  value={formData.username}
                  onChange={(e) => updateField('username', e.target.value.toLowerCase().replace(/[^a-z0-9_]/g, ''))}
                  placeholder="myaibot"
                  className="glass-input"
                />
                <p className="text-xs text-[var(--text-muted)] mt-1">
                  Lowercase letters, numbers, and underscores only
                </p>
              </div>

              <div>
                <label className="block text-sm font-medium text-[var(--text-secondary)] mb-2">
                  <div className="flex items-center gap-2">
                    <Cpu className="w-4 h-4" />
                    AI Model
                  </div>
                </label>
                <select
                  value={formData.model}
                  onChange={(e) => updateField('model', e.target.value)}
                  className="glass-input"
                >
                  {AVAILABLE_MODELS.map((model) => (
                    <option key={model.id} value={model.id}>
                      {model.name} - {model.description}
                    </option>
                  ))}
                </select>
              </div>

              <div>
                <label className="block text-sm font-medium text-[var(--text-secondary)] mb-2">
                  <div className="flex items-center gap-2">
                    <MessageSquare className="w-4 h-4" />
                    System Prompt
                  </div>
                </label>
                <textarea
                  value={formData.systemPrompt}
                  onChange={(e) => updateField('systemPrompt', e.target.value)}
                  placeholder="Enter a system prompt for your AI..."
                  rows={4}
                  className="glass-input resize-none"
                />
                <p className="text-xs text-[var(--text-muted)] mt-1">
                  This prompt defines your bot's personality and capabilities
                </p>
              </div>
            </div>

            {error && (
              <div className="flex items-center gap-2 p-3 rounded-lg bg-red-500/10 text-red-400 text-sm">
                <AlertCircle className="w-4 h-4 flex-shrink-0" />
                {error}
              </div>
            )}

            <div className="flex gap-3">
              <button
                onClick={() => {
                  setStep('complete')
                  onComplete?.()
                }}
                className="glass-button flex-1 py-3"
              >
                Skip for Now
              </button>
              <button
                onClick={handleConfigure}
                disabled={loading}
                className="glass-button glass-button-primary flex-1 py-3 flex items-center justify-center gap-2"
              >
                {loading ? (
                  <Loader2 className="w-5 h-5 animate-spin" />
                ) : (
                  <>
                    <Check className="w-5 h-5" />
                    Save Configuration
                  </>
                )}
              </button>
            </div>
          </motion.div>
        )}

        {/* Step 4: Complete */}
        {step === 'complete' && (
          <motion.div
            key="complete"
            initial={{ opacity: 0, scale: 0.95 }}
            animate={{ opacity: 1, scale: 1 }}
            className="text-center py-8"
          >
            <div className="w-20 h-20 rounded-full bg-gradient-to-br from-[var(--accent-start)] to-[var(--accent-end)] flex items-center justify-center mx-auto mb-6">
              <Check className="w-10 h-10 text-white" />
            </div>
            <h3 className="text-2xl font-semibold text-[var(--text-primary)] mb-3">
              Bot Created!
            </h3>
            <p className="text-[var(--text-muted)] mb-6">
              Your AI bot is now ready at {formData.phoneNumber}
            </p>
            <a
              href={`https://signal.me/#p/${formData.phoneNumber.replace(/[^+\d]/g, '')}`}
              target="_blank"
              rel="noopener noreferrer"
              className="glass-button glass-button-primary inline-flex items-center gap-2 px-6 py-3"
            >
              <MessageSquare className="w-5 h-5" />
              Open in Signal
            </a>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  )
}

export function RegistrationFormSkeleton() {
  return (
    <div className="glass-card p-8 md:p-10">
      <div className="flex items-center justify-center gap-2 mb-8">
        {[1, 2, 3, 4].map((i) => (
          <div key={i} className="flex items-center">
            <div className="w-8 h-8 rounded-full skeleton" />
            {i < 4 && <div className="w-12 h-0.5 mx-1 skeleton" />}
          </div>
        ))}
      </div>
      <div className="space-y-4">
        <div className="h-6 w-48 skeleton mx-auto" />
        <div className="h-4 w-64 skeleton mx-auto" />
        <div className="h-12 skeleton rounded-xl mt-6" />
        <div className="h-24 skeleton rounded-xl" />
        <div className="h-12 skeleton rounded-xl" />
      </div>
    </div>
  )
}
