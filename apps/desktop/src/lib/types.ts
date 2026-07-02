export type FlowStatus = "idle" | "listening" | "transcribing";

export interface Config {
  model_path: string | null;
  input_device: string | null;
  language: string;
  auto_type: boolean;
  copy_to_clipboard: boolean;
  show_metrics: boolean;
  metrics_corner: string;
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
