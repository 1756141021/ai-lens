import { invoke } from "@tauri-apps/api/core";
import type { ApiConfig } from "./api";

export interface AppConfig {
  api: {
    provider: string;
    base_url: string;
    api_key: string;
    model: string;
    supports_vision: boolean;
    auth_header: string;
    api_version: string;
    models: string[];
  };
  hotkey: string;
  cache: { max_count: number };
  ocr: { language: string };
  cursor: { enabled: boolean; radius: number; opacity: number; color: string };
  web: { enabled: boolean; mode: string; tavily_key: string };
}

export async function loadConfig(): Promise<AppConfig> {
  return invoke("get_config");
}

export async function saveConfig(config: AppConfig): Promise<void> {
  return invoke("set_config", { config });
}

export function toApiWeb(web: AppConfig["web"] | undefined): ApiConfig["web"] {
  return web?.enabled
    ? { mode: web.mode === "app" ? "app" : "native", tavilyKey: web.tavily_key }
    : undefined;
}

export function toApiConfig(config: AppConfig): ApiConfig {
  return {
    provider: (config.api.provider || "openai") as ApiConfig["provider"],
    baseUrl: config.api.base_url,
    apiKey: config.api.api_key,
    model: config.api.model,
    supportsVision: config.api.supports_vision,
    authHeader: config.api.auth_header,
    apiVersion: config.api.api_version,
    web: toApiWeb(config.web),
  };
}
