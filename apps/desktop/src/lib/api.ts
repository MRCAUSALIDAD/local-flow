import { invoke } from "@tauri-apps/api/core";
import type {
  Config,
  ExportFormat,
  LiveEntry,
  LoopbackSource,
  ModelInfo,
} from "./types";

export const getConfig = () => invoke<Config>("get_config");

export const setConfig = (config: Config) => invoke<void>("set_config", { config });

export const modelLoaded = () => invoke<boolean>("model_loaded");

export const listMicrophones = () => invoke<string[]>("list_microphones");

export const listModels = () => invoke<ModelInfo[]>("list_models");

export const downloadModel = (id: string) => invoke<void>("download_model", { id });

export const accessibilityOk = () => invoke<boolean>("accessibility_ok");

export const requestAccessibility = () => invoke<boolean>("request_accessibility");

export const openAccessibilitySettings = () =>
  invoke<void>("open_accessibility_settings");

export const startDictation = () => invoke<void>("start_dictation");

export const stopDictation = () => invoke<void>("stop_dictation");

/** Null when system audio capture works here, otherwise why it does not. */
export const systemAudioStatus = () => invoke<string | null>("system_audio_status");

export const listLoopbackSources = () =>
  invoke<LoopbackSource[]>("list_loopback_sources");

export const startListening = () => invoke<void>("start_listening");

export const stopListening = () => invoke<void>("stop_listening");

export const liveEntries = () => invoke<LiveEntry[]>("live_entries");

export const clearSession = () => invoke<void>("clear_session");

export const sessionText = () => invoke<string>("session_text");

/** Writes the transcript and resolves with the file path. */
export const exportSession = (format: ExportFormat) =>
  invoke<string>("export_session", { format });
