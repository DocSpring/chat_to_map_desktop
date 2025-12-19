import { invoke } from '@tauri-apps/api/core'
import type { ChatInfo, ScreenshotConfig } from './types'

// Mock data for screenshot mode
export function getMockChats(): ChatInfo[] {
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

export function setTheme(theme: string): void {
  if (theme === 'light' || theme === 'dark') {
    document.documentElement.setAttribute('data-theme', theme)
  } else {
    document.documentElement.removeAttribute('data-theme')
  }
}

export async function takeScreenshot(filename: string): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 300))
  const path = await invoke<string>('take_screenshot', { filename })
  console.log(`Screenshot saved: ${path}`)
}

interface ScreenshotContext {
  elements: {
    permissionScreen: HTMLElement
    chatSelectionScreen: HTMLElement
    progressScreen: HTMLElement
    successScreen: HTMLElement
    errorScreen: HTMLElement
    progressStage: HTMLElement
    progressFill: HTMLElement
    progressMessage: HTMLElement
    errorMessage: HTMLElement
  }
  state: { chats: ChatInfo[]; selectedIds: Set<number> }
  showScreen: (screen: HTMLElement) => void
  renderChatList: () => void
}

export async function runScreenshotMode(
  config: ScreenshotConfig,
  ctx: ScreenshotContext
): Promise<void> {
  console.log('Running screenshot mode...', config)
  setTheme(config.theme)

  const themeSuffix = config.theme === 'system' ? 'system' : config.theme

  if (config.force_no_fda) {
    ctx.showScreen(ctx.elements.permissionScreen)
    await takeScreenshot(`01-permission-${themeSuffix}.png`)
  }

  ctx.showScreen(ctx.elements.chatSelectionScreen)

  if (!config.force_no_fda) {
    try {
      ctx.state.chats = await invoke<ChatInfo[]>('list_chats')
    } catch {
      ctx.state.chats = getMockChats()
    }
  } else {
    ctx.state.chats = getMockChats()
  }

  ctx.renderChatList()
  await takeScreenshot(`02-chat-selection-empty-${themeSuffix}.png`)

  if (ctx.state.chats.length > 0) {
    ctx.state.selectedIds.add(ctx.state.chats[0].id)
    ctx.renderChatList()
    await takeScreenshot(`03-chat-selection-selected-${themeSuffix}.png`)
  }

  ctx.showScreen(ctx.elements.progressScreen)
  ctx.elements.progressStage.textContent = 'Exporting'
  ctx.elements.progressFill.style.width = '35%'
  ctx.elements.progressMessage.textContent = 'Exporting messages...'
  await takeScreenshot(`04-progress-${themeSuffix}.png`)

  ctx.showScreen(ctx.elements.successScreen)
  await takeScreenshot(`05-success-${themeSuffix}.png`)

  ctx.showScreen(ctx.elements.errorScreen)
  ctx.elements.errorMessage.textContent = 'Connection failed: Unable to reach server'
  await takeScreenshot(`06-error-${themeSuffix}.png`)

  console.log('Screenshot mode complete! All screenshots saved.')
}
