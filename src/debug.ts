/**
 * Debug panel functionality - hidden debug settings activated by 5 clicks on logo
 */

import { invoke } from '@tauri-apps/api/core'

// Constants
const DEBUG_HOST_KEY = 'chattomap_debug_host'
const DEBUG_HEADERS_KEY = 'chattomap_debug_headers'
const CLICK_THRESHOLD = 5
const CLICK_TIMEOUT_MS = 1000

// Types
export interface DebugHeader {
  name: string
  value: string
}

// State
let logoClickCount = 0
let lastLogoClickTime = 0

// Elements (initialized in setup)
let debugPanel: HTMLElement
let debugHostInput: HTMLInputElement
let debugHeadersList: HTMLElement
let debugAddHeaderBtn: HTMLButtonElement
let debugSaveBtn: HTMLButtonElement
let debugCloseBtn: HTMLButtonElement
let headerLogo: HTMLImageElement

function escapeHtml(text: string): string {
  const div = document.createElement('div')
  div.textContent = text
  return div.innerHTML
}

export function getDebugHeaders(): DebugHeader[] {
  const saved = localStorage.getItem(DEBUG_HEADERS_KEY)
  if (!saved) return []
  try {
    return JSON.parse(saved) as DebugHeader[]
  } catch {
    return []
  }
}

function renderDebugHeaders(): void {
  const headers = getDebugHeaders()
  debugHeadersList.innerHTML = ''

  for (let i = 0; i < headers.length; i++) {
    const header = headers[i]
    const row = document.createElement('div')
    row.className = 'debug-header-row'
    row.innerHTML = `
      <input type="text" placeholder="Header-Name" value="${escapeHtml(header.name)}" data-index="${i}" data-field="name" />
      <input type="text" placeholder="value" value="${escapeHtml(header.value)}" data-index="${i}" data-field="value" />
      <button type="button" class="btn-remove" data-index="${i}">&minus;</button>
    `
    debugHeadersList.appendChild(row)
  }

  // Add event listeners for inputs and remove buttons
  for (const input of debugHeadersList.querySelectorAll('input')) {
    input.addEventListener('input', handleHeaderInputChange)
  }
  for (const btn of debugHeadersList.querySelectorAll('.btn-remove')) {
    btn.addEventListener('click', handleRemoveHeader)
  }
}

function handleHeaderInputChange(e: Event): void {
  const input = e.target as HTMLInputElement
  const index = Number.parseInt(input.dataset['index'] ?? '0', 10)
  const field = input.dataset['field'] as 'name' | 'value'

  const headers = getDebugHeaders()
  if (headers[index]) {
    headers[index][field] = input.value
    localStorage.setItem(DEBUG_HEADERS_KEY, JSON.stringify(headers))
  }
}

function handleRemoveHeader(e: Event): void {
  const btn = e.target as HTMLButtonElement
  const index = Number.parseInt(btn.dataset['index'] ?? '0', 10)

  const headers = getDebugHeaders()
  headers.splice(index, 1)
  localStorage.setItem(DEBUG_HEADERS_KEY, JSON.stringify(headers))
  renderDebugHeaders()
}

function handleAddHeader(): void {
  const headers = getDebugHeaders()
  headers.push({ name: '', value: '' })
  localStorage.setItem(DEBUG_HEADERS_KEY, JSON.stringify(headers))
  renderDebugHeaders()

  // Focus the new name input
  const lastRow = debugHeadersList.lastElementChild
  if (lastRow) {
    const nameInput = lastRow.querySelector('input') as HTMLInputElement
    nameInput?.focus()
  }
}

function handleLogoClick(): void {
  const now = Date.now()

  // Reset counter if too much time has passed since last click
  if (now - lastLogoClickTime > CLICK_TIMEOUT_MS) {
    logoClickCount = 0
  }

  logoClickCount++
  lastLogoClickTime = now

  if (logoClickCount >= CLICK_THRESHOLD) {
    logoClickCount = 0
    debugPanel.classList.remove('hidden')
  }
}

async function saveDebugSettings(): Promise<void> {
  const url = debugHostInput.value.trim()

  if (url) {
    localStorage.setItem(DEBUG_HOST_KEY, url)
  } else {
    localStorage.removeItem(DEBUG_HOST_KEY)
  }

  // Get headers and filter out empty ones
  const headers = getDebugHeaders().filter((h) => h.name.trim() && h.value.trim())
  if (headers.length > 0) {
    localStorage.setItem(DEBUG_HEADERS_KEY, JSON.stringify(headers))
  } else {
    localStorage.removeItem(DEBUG_HEADERS_KEY)
  }

  // Convert headers to object for Rust
  const headersObj: Record<string, string> = {}
  for (const h of headers) {
    headersObj[h.name.trim()] = h.value.trim()
  }

  // Notify Rust about the new settings
  await invoke('set_server_host', { host: url || null })
  await invoke('set_custom_headers', { headers: headersObj })

  // Close panel and show confirmation
  debugPanel.classList.add('hidden')
  alert('Debug settings saved.\n\nThe app will now reload.')

  // Reload to apply changes
  window.location.reload()
}

export function setupDebugPanel(elements: {
  headerLogo: HTMLImageElement
  debugPanel: HTMLElement
  debugCloseBtn: HTMLButtonElement
  debugHostInput: HTMLInputElement
  debugHeadersList: HTMLElement
  debugAddHeaderBtn: HTMLButtonElement
  debugSaveBtn: HTMLButtonElement
}): void {
  // Store element references
  headerLogo = elements.headerLogo
  debugPanel = elements.debugPanel
  debugCloseBtn = elements.debugCloseBtn
  debugHostInput = elements.debugHostInput
  debugHeadersList = elements.debugHeadersList
  debugAddHeaderBtn = elements.debugAddHeaderBtn
  debugSaveBtn = elements.debugSaveBtn

  // Logo click detection
  headerLogo.addEventListener('click', handleLogoClick)

  // Debug panel controls
  debugCloseBtn.addEventListener('click', () => {
    debugPanel.classList.add('hidden')
  })

  debugSaveBtn.addEventListener('click', saveDebugSettings)

  // Debug shortcut links
  for (const link of document.querySelectorAll('.debug-shortcut')) {
    link.addEventListener('click', (e) => {
      e.preventDefault()
      const url = (e.target as HTMLElement).dataset['url']
      if (url) {
        debugHostInput.value = url
      }
    })
  }

  // Initialize debug host input with saved value
  const savedHost = localStorage.getItem(DEBUG_HOST_KEY)
  if (savedHost) {
    debugHostInput.value = savedHost
  }

  // Initialize debug headers
  debugAddHeaderBtn.addEventListener('click', handleAddHeader)
  renderDebugHeaders()
}

export async function initDebugSettingsOnStartup(): Promise<void> {
  // Initialize debug settings from localStorage
  const savedHost = localStorage.getItem(DEBUG_HOST_KEY)
  if (savedHost) {
    await invoke('set_server_host', { host: savedHost })
  }

  const savedHeaders = getDebugHeaders().filter((h) => h.name.trim() && h.value.trim())
  if (savedHeaders.length > 0) {
    const headersObj: Record<string, string> = {}
    for (const h of savedHeaders) {
      headersObj[h.name.trim()] = h.value.trim()
    }
    await invoke('set_custom_headers', { headers: headersObj })
  }
}
