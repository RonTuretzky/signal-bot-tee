import { motion } from 'framer-motion'
import { Check, Zap, Crown, MessageSquare } from 'lucide-react'

const plans = [
  {
    name: 'Free',
    price: '$0',
    period: 'forever',
    description: 'Try it out',
    features: [
      '15 messages per day',
      'DeepSeek V3.1 model',
      'TEE-encrypted conversations',
      'Web search, weather & calculator',
    ],
    cta: 'Register a Bot',
    ctaAction: 'scroll',
    highlighted: false,
    icon: Zap,
  },
  {
    name: 'Premium',
    price: '$19',
    period: '/month',
    description: 'For daily use',
    features: [
      'Unlimited messages',
      'All AI models',
      'TEE-encrypted conversations',
      'All tools included',
      'Priority response times',
    ],
    cta: 'Upgrade via Signal',
    ctaAction: 'signal',
    highlighted: true,
    icon: Crown,
  },
]

export function PricingTable() {
  return (
    <div className="grid grid-cols-1 sm:grid-cols-2 gap-6 max-w-2xl mx-auto">
      {plans.map((plan, index) => (
        <motion.div
          key={plan.name}
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.4, delay: index * 0.1 }}
          className={`glass-card p-6 relative ${
            plan.highlighted
              ? 'border-[var(--accent-start)]/30 bg-[var(--accent-start)]/[0.03]'
              : ''
          }`}
        >
          {plan.highlighted && (
            <div className="absolute -top-3 left-1/2 -translate-x-1/2 px-3 py-0.5 rounded-full bg-gradient-to-r from-[var(--accent-start)] to-[var(--accent-end)] text-xs font-semibold text-white">
              Popular
            </div>
          )}

          <div className="text-center mb-5">
            <plan.icon className={`w-8 h-8 mx-auto mb-3 ${
              plan.highlighted ? 'text-[var(--accent-start)]' : 'text-[var(--text-muted)]'
            }`} />
            <h3 className="text-lg font-semibold text-[var(--text-primary)]">{plan.name}</h3>
            <div className="mt-2">
              <span className="text-3xl font-bold text-[var(--text-primary)]">{plan.price}</span>
              <span className="text-sm text-[var(--text-muted)]">{plan.period}</span>
            </div>
            <p className="text-sm text-[var(--text-muted)] mt-1">{plan.description}</p>
          </div>

          <ul className="space-y-2.5 mb-6">
            {plan.features.map((feature) => (
              <li key={feature} className="flex items-center gap-2 text-sm text-[var(--text-secondary)]">
                <Check className="w-4 h-4 text-emerald-400 flex-shrink-0" />
                {feature}
              </li>
            ))}
          </ul>

          {plan.ctaAction === 'scroll' ? (
            <button
              onClick={() => document.getElementById('register')?.scrollIntoView({ behavior: 'smooth' })}
              className="glass-button w-full py-2.5 text-sm flex items-center justify-center gap-2"
            >
              {plan.cta}
            </button>
          ) : (
            <div className="text-center">
              <div className="flex items-center justify-center gap-1.5 text-sm text-[var(--text-muted)]">
                <MessageSquare className="w-3.5 h-3.5" />
                <span>Send <code className="bg-white/5 px-1.5 py-0.5 rounded text-xs">!subscribe</code> in Signal</span>
              </div>
            </div>
          )}
        </motion.div>
      ))}
    </div>
  )
}
