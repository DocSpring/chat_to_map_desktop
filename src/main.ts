import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open as openPath } from '@tauri-apps/plugin-dialog'
import { open as openShell } from '@tauri-apps/plugin-shell'
import tippy from 'tippy.js'
import 'tippy.js/dist/tippy.css'
import { FunnelEvents, initAnalytics, trackPageView } from './analytics'
import { initDebugSettingsOnStartup, setupDebugPanel } from './debug'
import { runScreenshotMode } from './screenshot'
import type { ChatInfo, ExportProgress, ExportResult, ScreenshotConfig } from './types'

// State
const state = {
  chats: [] as ChatInfo[],
  selectedIds: new Set<number>(),
  filter: '',
  lastResultsUrl: null as string | null,
  customDbPath: null as string | null
}

// Helper to get required DOM element
function getElement<T extends HTMLElement>(id: string): T {
  const el = document.getElementById(id)
  if (!el) {
    throw new Error(`Required element #${id} not found`)
  }
  return el as T
}

// DOM Elements
const elements = {
  permissionScreen: getElement<HTMLElement>('permission-screen'),
  chatSelectionScreen: getElement<HTMLElement>('chat-selection-screen'),
  progressScreen: getElement<HTMLElement>('progress-screen'),
  successScreen: getElement<HTMLElement>('success-screen'),
  errorScreen: getElement<HTMLElement>('error-screen'),

  filterInput: getElement<HTMLInputElement>('filter-input'),
  chatList: getElement<HTMLElement>('chat-list'),
  selectedCount: getElement<HTMLElement>('selected-count'),

  selectAllBtn: getElement<HTMLButtonElement>('select-all-btn'),
  selectNoneBtn: getElement<HTMLButtonElement>('select-none-btn'),
  exportBtn: getElement<HTMLButtonElement>('export-btn'),

  // Permission screen elements
  fdaStatus: getElement<HTMLElement>('fda-status'),
  contactsStatus: getElement<HTMLElement>('contacts-status'),
  openFdaSettingsBtn: getElement<HTMLButtonElement>('open-fda-settings-btn'),
  openContactsSettingsBtn: getElement<HTMLButtonElement>('open-contacts-settings-btn'),
  retryPermissionBtn: getElement<HTMLButtonElement>('retry-permission-btn'),
  selectDbBtn: getElement<HTMLButtonElement>('select-db-btn'),

  progressStage: getElement<HTMLElement>('progress-stage'),
  progressFill: getElement<HTMLElement>('progress-fill'),
  progressMessage: getElement<HTMLElement>('progress-message'),

  openResultsBtn: getElement<HTMLButtonElement>('open-results-btn'),

  errorMessage: getElement<HTMLElement>('error-message'),
  retryBtn: getElement<HTMLButtonElement>('retry-btn'),

  // Debug panel
  headerLogo: getElement<HTMLImageElement>('header-logo'),
  debugPanel: getElement<HTMLElement>('debug-panel'),
  debugCloseBtn: getElement<HTMLButtonElement>('debug-close-btn'),
  debugHostInput: getElement<HTMLInputElement>('debug-host-input'),
  debugHeadersList: getElement<HTMLElement>('debug-headers-list'),
  debugAddHeaderBtn: getElement<HTMLButtonElement>('debug-add-header-btn'),
  debugSaveBtn: getElement<HTMLButtonElement>('debug-save-btn')
}

// Screen management
function showScreen(screen: HTMLElement): void {
  const screens = [
    elements.permissionScreen,
    elements.chatSelectionScreen,
    elements.progressScreen,
    elements.successScreen,
    elements.errorScreen
  ]

  for (const s of screens) {
    s.classList.add('hidden')
  }
  screen.classList.remove('hidden')

  // Track page view for analytics
  trackPageView(screen.id)
}

// Get filtered chats based on current filter
function getFilteredChats(): ChatInfo[] {
  return state.chats.filter((chat) => {
    if (!state.filter) return true
    return chat.display_name.toLowerCase().includes(state.filter.toLowerCase())
  })
}

// Chat list rendering
function renderChatList(): void {
  const filteredChats = getFilteredChats()

  if (filteredChats.length === 0) {
    elements.chatList.innerHTML = '<div class="loading">No chats found</div>'
    return
  }

  elements.chatList.innerHTML = filteredChats
    .map((chat) => {
      const selected = state.selectedIds.has(chat.id)
      return `
        <div class="chat-item ${selected ? 'selected' : ''}" data-id="${chat.id}">
          <div class="chat-checkbox">${selected ? '✓' : ''}</div>
          <div class="chat-info">
            <div class="chat-name">${escapeHtml(chat.display_name)}</div>
            <div class="chat-meta">${chat.message_count} messages · ${escapeHtml(chat.service)}</div>
          </div>
        </div>
      `
    })
    .join('')

  updateSelectedCount()
}

function updateSelectedCount(): void {
  const count = state.selectedIds.size
  elements.selectedCount.textContent = `${count} chat${count === 1 ? '' : 's'} selected`
}

function escapeHtml(text: string): string {
  const div = document.createElement('div')
  div.textContent = text
  return div.innerHTML
}

// Event handlers
function setupEventListeners(): void {
  // Filter input
  elements.filterInput.addEventListener('input', () => {
    state.filter = elements.filterInput.value
    renderChatList()
  })

  // Chat list clicks
  elements.chatList.addEventListener('click', (e) => {
    const target = e.target as HTMLElement
    const chatItem = target.closest('.chat-item') as HTMLElement | null
    if (!chatItem) return

    const idStr = chatItem.dataset['id']
    if (!idStr) return

    const id = Number.parseInt(idStr, 10)
    if (Number.isNaN(id)) return

    if (state.selectedIds.has(id)) {
      state.selectedIds.delete(id)
    } else {
      state.selectedIds.add(id)
    }
    renderChatList()
  })

  // Select all/none
  elements.selectAllBtn.addEventListener('click', () => {
    for (const chat of getFilteredChats()) {
      state.selectedIds.add(chat.id)
    }
    renderChatList()
  })

  elements.selectNoneBtn.addEventListener('click', () => {
    for (const chat of getFilteredChats()) {
      state.selectedIds.delete(chat.id)
    }
    renderChatList()
  })

  // Export button
  elements.exportBtn.addEventListener('click', handleExport)

  // Permission screen buttons
  elements.openFdaSettingsBtn.addEventListener('click', async () => {
    FunnelEvents.openedSystemPreferences()
    await invoke('open_full_disk_access_settings')
  })

  elements.openContactsSettingsBtn.addEventListener('click', async () => {
    await invoke('open_contacts_settings')
  })

  elements.retryPermissionBtn.addEventListener('click', () => {
    FunnelEvents.retriedPermission()
    checkPermissionAndLoadChats()
  })

  elements.selectDbBtn.addEventListener('click', handleSelectCustomDb)

  // Success screen
  elements.openResultsBtn.addEventListener('click', () => {
    if (state.lastResultsUrl) {
      FunnelEvents.openedResults()
      openShell(state.lastResultsUrl)
    }
  })

  // Error screen
  elements.retryBtn.addEventListener('click', handleExport)

  // Setup debug panel
  setupDebugPanel(elements)

  // Handle external links (open in system browser)
  document.querySelectorAll('a[target="_blank"]').forEach((link) => {
    link.addEventListener('click', (e) => {
      e.preventDefault()
      const href = (e.currentTarget as HTMLAnchorElement).href
      if (href) {
        openShell(href)
      }
    })
  })
}

async function handleSelectCustomDb(): Promise<void> {
  try {
    const selected = await openPath({
      multiple: false,
      directory: false,
      filters: [
        {
          name: 'SQLite Database',
          extensions: ['db', 'sqlite', 'sqlite3']
        }
      ],
      title: 'Select your chat.db file'
    })

    if (selected && typeof selected === 'string') {
      // Validate it's a chat.db file
      const isValid = await invoke<boolean>('validate_chat_db', { path: selected })
      if (!isValid) {
        alert(
          'This does not appear to be a valid iMessage database.\n\n' +
            'The file should be named "chat.db" and contain iMessage tables.'
        )
        return
      }

      state.customDbPath = selected
      FunnelEvents.selectedCustomDb()
      showScreen(elements.chatSelectionScreen)
      await loadChats()
    }
  } catch (error) {
    console.error('Error selecting database:', error)
    alert(`Error selecting database: ${error}`)
  }
}

// Update permission status indicators in the UI
function updatePermissionStatus(element: HTMLElement, granted: boolean | null): void {
  const icon = element.querySelector('.status-icon')
  if (!icon) return

  icon.classList.remove('status-pending', 'status-granted', 'status-denied')
  if (granted === null) {
    icon.classList.add('status-pending')
    icon.textContent = '○'
  } else if (granted) {
    icon.classList.add('status-granted')
    icon.textContent = '✓'
  } else {
    icon.classList.add('status-denied')
    icon.textContent = '✗'
  }
}

async function checkPermissionAndLoadChats(): Promise<void> {
  console.log('[checkPermissionAndLoadChats] Starting...')
  try {
    // Check Full Disk Access
    console.log('[checkPermissionAndLoadChats] Invoking check_full_disk_access...')
    const hasFdaAccess = await invoke<boolean>('check_full_disk_access')
    console.log('[checkPermissionAndLoadChats] hasFdaAccess:', hasFdaAccess)

    // Check Contacts access
    console.log('[checkPermissionAndLoadChats] Invoking check_contacts_access...')
    const hasContactsAccess = await invoke<boolean>('check_contacts_access')
    console.log('[checkPermissionAndLoadChats] hasContactsAccess:', hasContactsAccess)

    // Update permission status UI
    updatePermissionStatus(elements.fdaStatus, hasFdaAccess)
    updatePermissionStatus(elements.contactsStatus, hasContactsAccess)

    // FDA is required - if not granted, show permission screen
    if (!hasFdaAccess) {
      FunnelEvents.permissionRequired()
      showScreen(elements.permissionScreen)
      return
    }

    // FDA granted - proceed to chat selection
    // (Contacts is optional - app works without it, just shows phone numbers)
    showScreen(elements.chatSelectionScreen)
    await loadChats()
  } catch (error) {
    console.error('Error checking permissions:', error)
    showError(String(error))
  }
}

async function loadChats(): Promise<void> {
  elements.chatList.innerHTML = '<div class="loading">Loading chats...</div>'

  try {
    state.chats = await invoke<ChatInfo[]>('list_chats', {
      customDbPath: state.customDbPath
    })
    FunnelEvents.chatsLoaded(state.chats.length)
    renderChatList()
  } catch (error) {
    console.error('Error loading chats:', error)
    elements.chatList.innerHTML = `<div class="loading">Error loading chats: ${escapeHtml(String(error))}</div>`
  }
}

async function handleExport(): Promise<void> {
  if (state.selectedIds.size === 0) {
    alert('Please select at least one chat to export.')
    return
  }

  FunnelEvents.exportStarted(state.selectedIds.size)
  showScreen(elements.progressScreen)

  try {
    const result = await invoke<ExportResult>('export_and_upload', {
      chatIds: Array.from(state.selectedIds),
      customDbPath: state.customDbPath
    })

    if (result.success && result.results_url) {
      FunnelEvents.exportCompleted(result.job_id ?? 'unknown')
      state.lastResultsUrl = result.results_url
      showScreen(elements.successScreen)
      // Browser is opened by Rust side
    } else {
      const errorMsg = result.error ?? 'Unknown error occurred'
      FunnelEvents.exportFailed(errorMsg)
      showError(errorMsg)
    }
  } catch (error) {
    console.error('Export error:', error)
    FunnelEvents.exportFailed(String(error))
    showError(String(error))
  }
}

function showError(message: string): void {
  elements.errorMessage.textContent = message
  showScreen(elements.errorScreen)
}

// Listen for progress updates from Rust
async function setupProgressListener(): Promise<void> {
  await listen<ExportProgress>('export-progress', (event) => {
    const progress = event.payload
    elements.progressStage.textContent = progress.stage
    elements.progressFill.style.width = `${progress.percent}%`
    elements.progressMessage.textContent = progress.message
  })
}

// Initialize tooltips
function initTooltips(): void {
  tippy('[data-tippy-content]', {
    placement: 'top',
    arrow: true,
    theme: 'light-border'
  })
}

// Initialize
async function init(): Promise<void> {
  initAnalytics()
  initTooltips()
  setupEventListeners()
  await setupProgressListener()
  await initDebugSettingsOnStartup()

  const config = await invoke<ScreenshotConfig>('get_screenshot_config')

  if (config.enabled) {
    await runScreenshotMode(config, {
      elements,
      state,
      showScreen,
      renderChatList
    })
  } else {
    await checkPermissionAndLoadChats()
  }
}

init()
