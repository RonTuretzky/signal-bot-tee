import { useState, useCallback, useEffect } from 'react'
import { login as apiLogin, logout as apiLogout, getStoredAuth } from '../lib/api'

export interface AuthState {
  isAuthenticated: boolean
  phone: string | null
  login: (phone: string, secret: string) => Promise<void>
  logout: () => void
  error: string | null
  loading: boolean
}

export function useAuth(): AuthState {
  const [phone, setPhone] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  // Restore auth from localStorage on mount
  useEffect(() => {
    const stored = getStoredAuth()
    if (stored) {
      setPhone(stored.phone)
    }
  }, [])

  const login = useCallback(async (phoneNumber: string, secret: string) => {
    setLoading(true)
    setError(null)
    try {
      const result = await apiLogin(phoneNumber, secret)
      setPhone(result.phone_number)
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Login failed'
      setError(message)
      throw err
    } finally {
      setLoading(false)
    }
  }, [])

  const logout = useCallback(() => {
    apiLogout()
    setPhone(null)
    setError(null)
  }, [])

  return {
    isAuthenticated: phone !== null,
    phone,
    login,
    logout,
    error,
    loading,
  }
}
