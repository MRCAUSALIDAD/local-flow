import { invoke } from "@tauri-apps/api/core";
import type { Config, ModelInfo } from "./types";

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
