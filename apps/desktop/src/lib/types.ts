export type FlowStatus =
  | "idle"
  | "listening"
  | "transcribing"
  | "listening-system";

/** Which stream a live segment came from. */
export type Track = "system" | "mic";

export interface LiveEntry {
  id: number;
  track: Track;
  text: string;
  start_ms: number;
  end_ms: number;
}

export interface LiveState {
  listening: boolean;
  elapsed_ms: number;
  backlog: number;
  lagging: boolean;
  dropped: number;
  /** False until audio arrives; a silent system is normal, not a failure. */
  receiving: boolean;
}

export interface LoopbackSource {
  name: string;
  is_default: boolean;
}

export type ExportFormat = "txt" | "md" | "srt";

export interface Config {
  model_path: string | null;
  input_device: string | null;
  language: string;
  auto_type: boolean;
  copy_to_clipboard: boolean;
  show_metrics: boolean;
  metrics_corner: string;
  loopback_source: string | null;
  capture_mic: boolean;
  vad_silence_ms: number;
  live_max_chunk_secs: number;
  suppress_mic_echo: boolean;
  live_partials: boolean;
}

export interface ModelInfo {
  id: string;
  label: string;
  note: string;
  size_mb: number;
  downloaded: boolean;
  active: boolean;
}

export interface TranscriptEntry {
  id: number;
  text: string;
  at: number;
}

export const METRICS_CORNERS: { value: string; label: string }[] = [
  { value: "top-right", label: "Arriba derecha" },
  { value: "top-left", label: "Arriba izquierda" },
  { value: "bottom-right", label: "Abajo derecha" },
  { value: "bottom-left", label: "Abajo izquierda" },
];

export const LANGUAGES: { code: string; label: string }[] = [
  { code: "auto", label: "Auto-detect" },
  { code: "en", label: "English" },
  { code: "es", label: "Español" },
  { code: "fr", label: "Français" },
  { code: "de", label: "Deutsch" },
  { code: "it", label: "Italiano" },
  { code: "pt", label: "Português" },
];
