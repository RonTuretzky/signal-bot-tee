import { useState, useEffect } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { motion } from 'framer-motion'
import { fetchDashboard, updateBotConfig, DashboardData, AVAILABLE_MODELS } from '../lib/api'
import { LogOut, MessageSquare, Cpu, Settings, CreditCard, Pencil, Save, X, Loader2 } from 'lucide-react'

interface DashboardProps {
  phone: string
  onLogout: () => void
}

export function Dashboard({ phone, onLogout }: DashboardProps) {
  const queryClient = useQueryClient()
  const { data, isLoading, error } = useQuery<DashboardData>({
    queryKey: ['dashboard', phone],
    queryFn: () => fetchDashboard(phone),
    staleTime: 30_000,
  })

  const [editing, setEditing] = useState(false)
  const [editModel, setEditModel] = useState('')
  const [editPrompt, setEditPrompt] = useState('')

  useEffect(() => {
    if (data) {
      setEditModel(data.model || AVAILABLE_MODELS[0].id)
      setEditPrompt(data.system_prompt || '')
    }
  }, [data])

  const saveMutation = useMutation({
    mutationFn: () =>
      updateBotConfig(phone, {
        model: editModel || undefined,
        system_prompt: editPrompt || undefined,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['dashboard', phone] })
      setEditing(false)
    },
  })

  if (isLoading) {
    return (
      <div className="glass-card p-8 space-y-4 animate-pulse">
        <div className="h-6 w-48 skeleton rounded" />
        <div className="h-4 w-64 skeleton rounded" />
        <div className="h-20 skeleton rounded" />
      </div>
    )
  }

  if (error || !data) {
    return (
      <div className="glass-card p-8 text-center">
        <p className="text-red-400 mb-4">{error instanceof Error ? error.message : 'Failed to load dashboard'}</p>
        <button onClick={onLogout} className="glass-button">Log Out</button>
      </div>
    )
  }

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className="space-y-6"
    >
      {/* Header */}
      <div className="glass-card p-6 flex items-center justify-between">
        <div>
          <h3 className="text-xl font-semibold text-[var(--text-primary)]">
            {data.username || data.phone_number}
          </h3>
          <p className="text-sm text-[var(--text-muted)]">{data.phone_number}</p>
        </div>
        <div className="flex items-center gap-3">
          <span className={`inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-medium ${
            data.status === 'verified'
              ? 'bg-emerald-500/10 text-emerald-400'
              : 'bg-amber-500/10 text-amber-400'
          }`}>
            <span className={`w-1.5 h-1.5 rounded-full ${
              data.status === 'verified' ? 'bg-emerald-500' : 'bg-amber-500'
            }`} />
            {data.status === 'verified' ? 'Online' : data.status}
          </span>
          <button onClick={onLogout} className="glass-button text-sm flex items-center gap-1.5 px-3 py-1.5">
            <LogOut className="w-3.5 h-3.5" />
            Log Out
          </button>
        </div>
      </div>

      {/* Quick Actions */}
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
        {data.signal_link && (
          <a
            href={data.signal_link}
            target="_blank"
            rel="noopener noreferrer"
            className="glass-card p-5 hover:border-[var(--accent-start)]/30 transition-colors group"
          >
            <div className="flex items-center gap-3 mb-2">
              <MessageSquare className="w-5 h-5 text-[var(--accent-start)] group-hover:text-[var(--accent-end)] transition-colors" />
              <span className="font-medium text-[var(--text-primary)]">Message</span>
            </div>
            <p className="text-xs text-[var(--text-muted)]">Open in Signal</p>
          </a>
        )}
        <div className="glass-card p-5">
          <div className="flex items-center gap-3 mb-2">
            <Cpu className="w-5 h-5 text-[var(--accent-mid)]" />
            <span className="font-medium text-[var(--text-primary)]">Model</span>
          </div>
          <p className="text-xs text-[var(--text-muted)] font-mono">
            {data.model?.split('/').pop() || 'Default'}
          </p>
        </div>
        <div className="glass-card p-5">
          <div className="flex items-center gap-3 mb-2">
            <CreditCard className="w-5 h-5 text-[var(--accent-end)]" />
            <span className="font-medium text-[var(--text-primary)]">Plan</span>
          </div>
          <p className="text-xs text-[var(--text-muted)]">
            Send <code className="bg-white/5 px-1 py-0.5 rounded">!subscription</code> in Signal
          </p>
        </div>
      </div>

      {/* Bot Config */}
      <div className="glass-card p-6">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <Settings className="w-4 h-4 text-[var(--text-muted)]" />
            <span className="text-sm font-medium text-[var(--text-secondary)]">Bot Configuration</span>
          </div>
          {!editing ? (
            <button
              onClick={() => setEditing(true)}
              className="glass-button text-xs flex items-center gap-1.5 px-3 py-1.5"
            >
              <Pencil className="w-3 h-3" />
              Edit
            </button>
          ) : (
            <div className="flex items-center gap-2">
              <button
                onClick={() => setEditing(false)}
                className="glass-button text-xs flex items-center gap-1.5 px-3 py-1.5"
                disabled={saveMutation.isPending}
              >
                <X className="w-3 h-3" />
                Cancel
              </button>
              <button
                onClick={() => saveMutation.mutate()}
                className="glass-button glass-button-primary text-xs flex items-center gap-1.5 px-3 py-1.5"
                disabled={saveMutation.isPending}
              >
                {saveMutation.isPending ? (
                  <Loader2 className="w-3 h-3 animate-spin" />
                ) : (
                  <Save className="w-3 h-3" />
                )}
                Save
              </button>
            </div>
          )}
        </div>

        {saveMutation.isError && (
          <div className="flex items-center gap-2 p-3 rounded-lg bg-red-500/10 text-red-400 text-sm mb-4">
            {saveMutation.error instanceof Error ? saveMutation.error.message : 'Save failed'}
          </div>
        )}

        {editing ? (
          <div className="space-y-4">
            <div>
              <label className="block text-xs font-medium text-[var(--text-muted)] mb-1.5">AI Model</label>
              <select
                value={editModel}
                onChange={(e) => setEditModel(e.target.value)}
                className="glass-input text-sm"
              >
                {AVAILABLE_MODELS.map((model) => (
                  <option key={model.id} value={model.id}>
                    {model.name} - {model.description}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="block text-xs font-medium text-[var(--text-muted)] mb-1.5">System Prompt</label>
              <textarea
                value={editPrompt}
                onChange={(e) => setEditPrompt(e.target.value)}
                rows={6}
                className="glass-input text-sm resize-none"
              />
            </div>
          </div>
        ) : (
          <div className="space-y-3">
            <div>
              <span className="text-xs text-[var(--text-muted)]">Model</span>
              <p className="text-sm text-[var(--text-secondary)] font-mono mt-0.5">
                {data.model || 'Default'}
              </p>
            </div>
            {data.system_prompt && (
              <div>
                <span className="text-xs text-[var(--text-muted)]">System Prompt</span>
                <p className="text-sm text-[var(--text-muted)] whitespace-pre-wrap leading-relaxed mt-0.5 max-h-40 overflow-y-auto">
                  {data.system_prompt}
                </p>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Info */}
      <div className="text-center text-xs text-[var(--text-muted)]">
        Registered {new Date(data.registered_at).toLocaleDateString()}
      </div>
    </motion.div>
  )
}
