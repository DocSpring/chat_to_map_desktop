import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open as openPath } from '@tauri-apps/plugin-dialog'
import { open as openShell } from '@tauri-apps/plugin-shell'
import { FunnelEvents, initAnalytics, trackPageView } from './analytics'

// Types matching Rust structs
interface ChatInfo {
  id: number
  display_name: string
  chat_identifier: string
  service: string
  participant_count: number
  message_count: number
}

interface ExportProgress {
  stage: string
  percent: number
  message: string
}

interface ExportResult {
  success: boolean
  job_id: string | null
  results_url: string | null
  error: string | null
}

interface ScreenshotConfig {
  enabled: boolean
  theme: string
  force_no_fda: boolean
  output_dir: string
}

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

  openSettingsBtn: getElement<HTMLButtonElement>('open-settings-btn'),
  retryPermissionBtn: getElement<HTMLButtonElement>('retry-permission-btn'),
  selectDbBtn: getElement<HTMLButtonElement>('select-db-btn'),

  progressStage: getElement<HTMLElement>('progress-stage'),
  progressFill: getElement<HTMLElement>('progress-fill'),
  progressMessage: getElement<HTMLElement>('progress-message'),

  openResultsBtn: getElement<HTMLButtonElement>('open-results-btn'),

  errorMessage: getElement<HTMLElement>('error-message'),
  retryBtn: getElement<HTMLButtonElement>('retry-btn')
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
  elements.openSettingsBtn.addEventListener('click', async () => {
    FunnelEvents.openedSystemPreferences()
    await invoke('open_full_disk_access_settings')
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

async function checkPermissionAndLoadChats(): Promise<void> {
  console.log('[checkPermissionAndLoadChats] Starting...')
  try {
    console.log('[checkPermissionAndLoadChats] Invoking check_full_disk_access...')
    const hasAccess = await invoke<boolean>('check_full_disk_access')
    console.log('[checkPermissionAndLoadChats] hasAccess:', hasAccess)

    if (!hasAccess) {
      FunnelEvents.permissionRequired()
      showScreen(elements.permissionScreen)
      return
    }

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

// Mock data for screenshot mode
function getMockChats(): ChatInfo[] {
  return [
    {
      id: 1,
      display_name: 'Alice Johnson',
      chat_identifier: '+15551234567',
      service: 'iMessage',
      participant_count: 1,
      message_count: 1542
    },
    {
      id: 2,
      display_name: 'Travel Planning Group',
      chat_identifier: 'chat123',
      service: 'iMessage',
      participant_count: 5,
      message_count: 823
    },
    {
      id: 3,
      display_name: 'Bob Williams',
      chat_identifier: '+15559876543',
      service: 'iMessage',
      participant_count: 1,
      message_count: 456
    }
  ]
}

// Screenshot mode functions
function setTheme(theme: string): void {
  if (theme === 'light' || theme === 'dark') {
    document.documentElement.setAttribute('data-theme', theme)
  } else {
    // 'system' - remove attribute to use system preference
    document.documentElement.removeAttribute('data-theme')
  }
}

async function takeScreenshot(filename: string): Promise<void> {
  // Small delay to ensure rendering is complete
  await new Promise((resolve) => setTimeout(resolve, 300))
  const path = await invoke<string>('take_screenshot', { filename })
  console.log(`Screenshot saved: ${path}`)
}

async function runScreenshotMode(config: ScreenshotConfig): Promise<void> {
  console.log('Running screenshot mode...')
  console.log('Config:', config)

  // Set theme
  setTheme(config.theme)

  // Determine theme suffix for filenames
  const themeSuffix = config.theme === 'system' ? 'system' : config.theme

  // Screen 1: Permission screen (if force_no_fda)
  if (config.force_no_fda) {
    showScreen(elements.permissionScreen)
    await takeScreenshot(`01-permission-${themeSuffix}.png`)
  }

  // Screen 2: Chat selection with chats loaded
  showScreen(elements.chatSelectionScreen)

  // Load real chats if we have FDA access, otherwise show mock data
  if (!config.force_no_fda) {
    try {
      state.chats = await invoke<ChatInfo[]>('list_chats')
    } catch {
      // If loading fails, use mock data for screenshots
      state.chats = getMockChats()
    }
  } else {
    // Mock data for permission-denied screenshots
    state.chats = getMockChats()
  }

  renderChatList()
  await takeScreenshot(`02-chat-selection-empty-${themeSuffix}.png`)

  // Select a chat
  if (state.chats.length > 0) {
    state.selectedIds.add(state.chats[0].id)
    renderChatList()
    await takeScreenshot(`03-chat-selection-selected-${themeSuffix}.png`)
  }

  // Screen 3: Progress screen
  showScreen(elements.progressScreen)
  elements.progressStage.textContent = 'Exporting'
  elements.progressFill.style.width = '35%'
  elements.progressMessage.textContent = 'Exporting messages...'
  await takeScreenshot(`04-progress-${themeSuffix}.png`)

  // Screen 4: Success screen
  showScreen(elements.successScreen)
  await takeScreenshot(`05-success-${themeSuffix}.png`)

  // Screen 5: Error screen
  showScreen(elements.errorScreen)
  elements.errorMessage.textContent = 'Connection failed: Unable to reach server'
  await takeScreenshot(`06-error-${themeSuffix}.png`)

  console.log('Screenshot mode complete!')

  // Exit the app after screenshots
  // Note: In Tauri, we can't easily exit from JS, so we just log completion
  console.log('All screenshots saved. You can close the app now.')
}

// Initialize
async function init(): Promise<void> {
  // Initialize analytics first
  initAnalytics()

  setupEventListeners()
  await setupProgressListener()

  // Check if we're in screenshot mode
  const config = await invoke<ScreenshotConfig>('get_screenshot_config')

  if (config.enabled) {
    await runScreenshotMode(config)
  } else {
    await checkPermissionAndLoadChats()
  }
}

init()
