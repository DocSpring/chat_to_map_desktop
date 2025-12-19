// Types matching Rust structs
export interface ChatInfo {
  id: number
  display_name: string
  chat_identifier: string
  service: string
  participant_count: number
  message_count: number
}

export interface ExportProgress {
  stage: string
  percent: number
  message: string
}

export interface ExportResult {
  success: boolean
  job_id: string | null
  results_url: string | null
  error: string | null
}

export interface ScreenshotConfig {
  enabled: boolean
  theme: string
  force_no_fda: boolean
  output_dir: string
}
