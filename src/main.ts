import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-shell'

// Types matching Rust structs
interface ChatInfo {
  id: string
  display_name: string
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
  error: string | null
}

// State
const state = {
  chats: [] as ChatInfo[],
  selectedIds: new Set<string>(),
  filter: '',
  lastJobId: null as string | null
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

  progressStage: getElement<HTMLElement>('progress-stage'),
  progressFill: getElement<HTMLElement>('progress-fill'),
  progressMessage: getElement<HTMLElement>('progress-message'),

  openResultsBtn: getElement<HTMLButtonElement>('open-results-btn'),

  errorMessage: getElement<HTMLElement>('error-message'),
  retryBtn: getElement<HTMLButtonElement>('retry-btn'),
  saveLocallyBtn: getElement<HTMLButtonElement>('save-locally-btn')
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
            <div class="chat-meta">${chat.message_count} messages</div>
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

    const id = chatItem.dataset['id']
    if (!id) return

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
    await invoke('open_full_disk_access_settings')
  })

  elements.retryPermissionBtn.addEventListener('click', checkPermissionAndLoadChats)

  // Success screen
  elements.openResultsBtn.addEventListener('click', () => {
    if (state.lastJobId) {
      open(`https://chattomap.com/processing/${state.lastJobId}`)
    }
  })

  // Error screen
  elements.retryBtn.addEventListener('click', handleExport)
  elements.saveLocallyBtn.addEventListener('click', handleSaveLocally)
}

async function checkPermissionAndLoadChats(): Promise<void> {
  try {
    const hasAccess = await invoke<boolean>('check_full_disk_access')

    if (!hasAccess) {
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
    state.chats = await invoke<ChatInfo[]>('list_chats')
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

  showScreen(elements.progressScreen)

  try {
    const result = await invoke<ExportResult>('export_and_upload', {
      chatIds: Array.from(state.selectedIds)
    })

    if (result.success && result.job_id) {
      state.lastJobId = result.job_id
      showScreen(elements.successScreen)

      // Open browser to results
      await open(`https://chattomap.com/processing/${result.job_id}`)
    } else {
      showError(result.error ?? 'Unknown error occurred')
    }
  } catch (error) {
    console.error('Export error:', error)
    showError(String(error))
  }
}

function handleSaveLocally(): void {
  // TODO: Implement save to local file
  alert('Save locally feature coming soon')
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

// Initialize
async function init(): Promise<void> {
  setupEventListeners()
  await setupProgressListener()
  await checkPermissionAndLoadChats()
}

init()
