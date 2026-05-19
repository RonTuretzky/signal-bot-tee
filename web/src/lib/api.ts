const API_URL = import.meta.env.VITE_API_URL || 'https://cf847538c919958236086a69ad1a2281103c7fee-8081.dstack-pha-prod9.phala.network'

export interface Bot {
  username: string
  phone_number: string
  signal_link: string
  identity_key?: string
  registered_at: string
  model?: string
  description?: string
  system_prompt?: string
}

export interface Attestation {
  in_tee: boolean
  app_id?: string
  compose_hash?: string
  tdx_quote_base64?: string
  verification_url?: string
  report_data?: string
}

export interface RegisterRequest {
  captcha?: string
  use_voice?: boolean
  ownership_secret?: string
  model?: string
  system_prompt?: string
  username?: string
}

export interface RegisterResponse {
  phone_number: string
  status: string
  message: string
}

export interface VerifyRequest {
  pin?: string
  ownership_secret?: string
}

export interface VerifyResponse {
  phone_number: string
  status: string
  message: string
}

export interface AccountInfo {
  phone_number: string
  status: 'pending' | 'verified' | 'failed'
  registered_at: string
  model?: string
  system_prompt?: string
  username?: string
}

export interface AccountsResponse {
  accounts: AccountInfo[]
  total: number
}

export interface LoginResponse {
  token: string
  phone_number: string
  expires_at: number
}

export interface DashboardData {
  phone_number: string
  status: string
  registered_at: string
  model?: string
  system_prompt?: string
  username?: string
  signal_link?: string
}

export interface ApiError {
  error: string
  code: string
}

// Available AI models
export const AVAILABLE_MODELS = [
  { id: 'deepseek-ai/DeepSeek-V3.1', name: 'DeepSeek V3.1', description: 'Fast and capable' },
  { id: 'openai/gpt-oss-120b', name: 'GPT OSS 120B', description: 'Large open source model' },
  { id: 'Qwen/Qwen3-30B-A3B-Instruct-2507', name: 'Qwen3 30B', description: 'Multilingual' },
  { id: 'zai-org/GLM-4.6', name: 'GLM 4.6', description: 'Chinese-optimized' },
] as const

// Helper to parse error responses with structured codes
async function parseError(response: Response): Promise<string> {
  try {
    const body: ApiError = await response.json()
    return body.error || body.code || response.statusText
  } catch {
    return response.statusText
  }
}

// Auth helpers
function getAuthHeader(): Record<string, string> {
  const token = localStorage.getItem('auth_token')
  if (token) {
    return { Authorization: `Bearer ${token}` }
  }
  return {}
}

export async function fetchBots(): Promise<Bot[]> {
  const response = await fetch(`${API_URL}/v1/bots`)
  if (!response.ok) {
    throw new Error(`Failed to fetch bots: ${response.statusText}`)
  }
  return response.json()
}

export async function fetchAccounts(): Promise<AccountsResponse> {
  const response = await fetch(`${API_URL}/v1/accounts`)
  if (!response.ok) {
    throw new Error(`Failed to fetch accounts: ${response.statusText}`)
  }
  return response.json()
}

export async function registerNumber(phoneNumber: string, request: RegisterRequest): Promise<RegisterResponse> {
  const response = await fetch(`${API_URL}/v1/register/${encodeURIComponent(phoneNumber)}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(request),
  })
  if (!response.ok) {
    throw new Error(await parseError(response))
  }
  return response.json()
}

export async function verifyRegistration(phoneNumber: string, code: string, request: VerifyRequest): Promise<VerifyResponse> {
  const response = await fetch(`${API_URL}/v1/register/${encodeURIComponent(phoneNumber)}/verify/${encodeURIComponent(code)}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(request),
  })
  if (!response.ok) {
    throw new Error(await parseError(response))
  }
  return response.json()
}

export async function setUsername(phoneNumber: string, username: string, ownershipSecret?: string): Promise<{ username?: string; username_link?: string }> {
  const response = await fetch(`${API_URL}/v1/accounts/${encodeURIComponent(phoneNumber)}/username`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, ownership_secret: ownershipSecret }),
  })
  if (!response.ok) {
    throw new Error(await parseError(response))
  }
  return response.json()
}

export async function updateProfile(phoneNumber: string, name?: string, about?: string, ownershipSecret?: string): Promise<void> {
  const response = await fetch(`${API_URL}/v1/profiles/${encodeURIComponent(phoneNumber)}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name, about, ownership_secret: ownershipSecret }),
  })
  if (!response.ok) {
    throw new Error(await parseError(response))
  }
}

export async function login(phoneNumber: string, ownershipSecret: string): Promise<LoginResponse> {
  const response = await fetch(`${API_URL}/v1/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ phone_number: phoneNumber, ownership_secret: ownershipSecret }),
  })
  if (!response.ok) {
    throw new Error(await parseError(response))
  }
  const data: LoginResponse = await response.json()
  localStorage.setItem('auth_token', data.token)
  localStorage.setItem('auth_phone', data.phone_number)
  localStorage.setItem('auth_expires', data.expires_at.toString())
  return data
}

export function logout(): void {
  localStorage.removeItem('auth_token')
  localStorage.removeItem('auth_phone')
  localStorage.removeItem('auth_expires')
}

export function getStoredAuth(): { phone: string; token: string } | null {
  const token = localStorage.getItem('auth_token')
  const phone = localStorage.getItem('auth_phone')
  const expires = localStorage.getItem('auth_expires')
  if (!token || !phone || !expires) return null
  if (Date.now() / 1000 > parseInt(expires)) {
    logout()
    return null
  }
  return { phone, token }
}

export async function fetchDashboard(phoneNumber: string): Promise<DashboardData> {
  const response = await fetch(`${API_URL}/v1/dashboard/${encodeURIComponent(phoneNumber)}`, {
    headers: { ...getAuthHeader() },
  })
  if (!response.ok) {
    throw new Error(await parseError(response))
  }
  return response.json()
}

export async function updateBotConfig(
  phoneNumber: string,
  config: { model?: string; system_prompt?: string }
): Promise<{ phone_number: string; model?: string; system_prompt?: string; message: string }> {
  const response = await fetch(`${API_URL}/v1/dashboard/${encodeURIComponent(phoneNumber)}/config`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json', ...getAuthHeader() },
    body: JSON.stringify(config),
  })
  if (!response.ok) {
    throw new Error(await parseError(response))
  }
  return response.json()
}

export async function fetchAttestation(): Promise<Attestation> {
  const response = await fetch(`${API_URL}/v1/attestation`)
  if (!response.ok) {
    throw new Error(`Failed to fetch attestation: ${response.statusText}`)
  }
  return response.json()
}

export async function fetchAttestationWithChallenge(challenge: string): Promise<Attestation> {
  const response = await fetch(`${API_URL}/v1/attestation?challenge=${encodeURIComponent(challenge)}`)
  if (!response.ok) {
    throw new Error(`Failed to fetch attestation: ${response.statusText}`)
  }
  return response.json()
}
