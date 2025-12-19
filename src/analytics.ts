/**
 * Google Analytics 4 tracking for ChatToMap Desktop
 *
 * Tracks virtual page views and funnel events in the Tauri app.
 * GA4 measurement ID is configured via VITE_GA_MEASUREMENT_ID env variable.
 */

// GA4 measurement ID - set via environment variable at build time
const GA_MEASUREMENT_ID = import.meta.env.VITE_GA_MEASUREMENT_ID || ''

// Screen name mapping for cleaner analytics
const SCREEN_NAMES: Record<string, string> = {
  'permission-screen': 'Permission Required',
  'chat-selection-screen': 'Chat Selection',
  'progress-screen': 'Export Progress',
  'success-screen': 'Export Complete',
  'error-screen': 'Error'
}

// Declare gtag on window
declare global {
  interface Window {
    dataLayer: unknown[]
    gtag: (...args: unknown[]) => void
  }
}

/**
 * Initialize Google Analytics
 * Called once on app startup
 */
export function initAnalytics(): void {
  if (!GA_MEASUREMENT_ID) {
    console.log('[Analytics] No measurement ID configured, analytics disabled')
    return
  }

  // Initialize dataLayer
  window.dataLayer = window.dataLayer || []
  window.gtag = function gtag(...args: unknown[]) {
    window.dataLayer.push(args)
  }

  // Load GA4 script
  const script = document.createElement('script')
  script.async = true
  script.src = `https://www.googletagmanager.com/gtag/js?id=${GA_MEASUREMENT_ID}`
  document.head.appendChild(script)

  // Configure GA4
  window.gtag('js', new Date())
  window.gtag('config', GA_MEASUREMENT_ID, {
    // Disable automatic page view tracking (we track manually)
    send_page_view: false,
    // Desktop app identifier
    app_name: 'ChatToMap Desktop',
    app_version: '0.1.0'
  })

  console.log('[Analytics] Initialized with ID:', GA_MEASUREMENT_ID)
}

/**
 * Track a virtual page view
 * Called when switching screens
 */
export function trackPageView(screenId: string): void {
  if (!GA_MEASUREMENT_ID || !window.gtag) return

  const screenName = SCREEN_NAMES[screenId] || screenId

  window.gtag('event', 'page_view', {
    page_title: screenName,
    page_location: `app://chattomap/${screenId}`,
    page_path: `/${screenId}`
  })

  console.log('[Analytics] Page view:', screenName)
}

/**
 * Track a custom event
 */
export function trackEvent(
  eventName: string,
  params?: Record<string, string | number | boolean>
): void {
  if (!GA_MEASUREMENT_ID || !window.gtag) return

  window.gtag('event', eventName, params)
  console.log('[Analytics] Event:', eventName, params)
}

// Funnel events for easy tracking
export const FunnelEvents = {
  /** User landed on permission screen */
  permissionRequired: () => trackEvent('funnel_permission_required'),

  /** User opened system preferences */
  openedSystemPreferences: () => trackEvent('funnel_opened_preferences'),

  /** User retried permission check */
  retriedPermission: () => trackEvent('funnel_retried_permission'),

  /** User selected a custom chat.db file (fallback path) */
  selectedCustomDb: () => trackEvent('funnel_selected_custom_db'),

  /** Chats loaded successfully */
  chatsLoaded: (count: number) => trackEvent('funnel_chats_loaded', { chat_count: count }),

  /** User selected chats and started export */
  exportStarted: (chatCount: number) =>
    trackEvent('funnel_export_started', { selected_chats: chatCount }),

  /** Export completed successfully */
  exportCompleted: (jobId: string) => trackEvent('funnel_export_completed', { job_id: jobId }),

  /** Export failed */
  exportFailed: (error: string) =>
    trackEvent('funnel_export_failed', { error: error.slice(0, 100) }),

  /** User opened results in browser */
  openedResults: () => trackEvent('funnel_opened_results')
}
