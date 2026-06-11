// Tauri's HTTP plugin routes requests through Rust, bypassing the webview's CORS
// (raw browser fetch to LLM APIs gets blocked → opaque "Failed to fetch").
import { fetch } from "@tauri-apps/plugin-http";

export type Provider = "openai" | "anthropic" | "gemini";

export interface ApiConfig {
  provider: Provider;
  baseUrl: string;
  apiKey: string;
  model: string;
  supportsVision: boolean;
  authHeader: string;
  apiVersion: string;
}

/** A neutral conversation turn. Adapters convert these into each provider's wire
 *  format — the chat layer never touches provider-specific shapes. */
export interface ChatTurn {
  role: "user" | "assistant";
  text: string;
  imageBase64?: string; // only on the turn that carried a screenshot
  ocrText?: string;     // only used when vision is off (text-only providers/models)
}

interface Request {
  url: string;
  headers: Record<string, string>;
  body: unknown;
}

// crop is always PNG (see capture.rs encode_png)
const MEDIA_TYPE = "image/png";

function trimSlash(s: string): string {
  return s.trim().replace(/\/+$/, "");
}

function withScheme(url: string): string {
  if (!url) return url;
  return /^https?:\/\//i.test(url) ? url : "https://" + url;
}

// ---------- OpenAI (covers OpenAI / DeepSeek / Ollama / OpenRouter / Azure) ----------

function openaiUrl(config: ApiConfig): string {
  let url = withScheme(trimSlash(config.baseUrl)) || "https://api.openai.com/v1";
  url = trimSlash(url);
  if (!/\/chat\/completions$/i.test(url)) {
    try {
      const u = new URL(url);
      if (u.pathname === "" || u.pathname === "/") url = `${u.origin}/v1/chat/completions`;
      else url = url + "/chat/completions";
    } catch {
      url = url + "/chat/completions";
    }
  }
  if (config.baseUrl.includes(".openai.azure.com") && !url.includes("api-version")) {
    url += (url.includes("?") ? "&" : "?") + `api-version=${config.apiVersion || "2024-10-21"}`;
  }
  return url;
}

function openaiHeaders(config: ApiConfig): Record<string, string> {
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  if (!config.apiKey) return headers;
  if (config.baseUrl.includes(".openai.azure.com")) {
    headers["api-key"] = config.apiKey;
  } else if (config.authHeader === "Authorization" || !config.authHeader) {
    headers["Authorization"] = `Bearer ${config.apiKey}`;
  } else {
    headers[config.authHeader] = config.apiKey;
  }
  return headers;
}

function openaiRequest(config: ApiConfig, turns: ChatTurn[]): Request {
  const messages = turns.map((t) => {
    if (t.role === "user" && t.imageBase64 && config.supportsVision) {
      return {
        role: "user",
        content: [
          { type: "image_url", image_url: { url: `data:${MEDIA_TYPE};base64,${t.imageBase64}` } },
          { type: "text", text: t.text || "What do you see in this image?" },
        ],
      };
    }
    const content =
      t.role === "user" && t.ocrText ? `[Image OCR text]:\n${t.ocrText}\n\n${t.text}` : t.text;
    return { role: t.role, content };
  });
  return {
    url: openaiUrl(config),
    headers: openaiHeaders(config),
    body: { model: config.model, messages, stream: true },
  };
}

// ---------- Anthropic (native Messages API) ----------

function anthropicRequest(config: ApiConfig, turns: ChatTurn[]): Request {
  const base = trimSlash(withScheme(config.baseUrl)) || "https://api.anthropic.com";
  const url = /\/v1\/messages$/i.test(base) ? base : `${base}/v1/messages`;

  const messages = turns.map((t) => {
    if (t.role === "user" && t.imageBase64 && config.supportsVision) {
      return {
        role: "user",
        content: [
          { type: "image", source: { type: "base64", media_type: MEDIA_TYPE, data: t.imageBase64 } },
          { type: "text", text: t.text || "What do you see in this image?" },
        ],
      };
    }
    const text = t.role === "user" && t.ocrText ? `[Image OCR text]:\n${t.ocrText}\n\n${t.text}` : t.text;
    return { role: t.role, content: text };
  });

  return {
    url,
    headers: {
      "Content-Type": "application/json",
      "x-api-key": config.apiKey,
      "anthropic-version": config.apiVersion?.trim() || "2023-06-01",
      // webview origin is a browser context; Anthropic blocks it without this.
      "anthropic-dangerous-direct-browser-access": "true",
    },
    body: { model: config.model, max_tokens: 4096, stream: true, messages },
  };
}

// ---------- Gemini (native generateContent, SSE) ----------

function geminiRequest(config: ApiConfig, turns: ChatTurn[]): Request {
  const base = trimSlash(withScheme(config.baseUrl)) || "https://generativelanguage.googleapis.com";
  const url = `${base}/v1beta/models/${config.model}:streamGenerateContent?alt=sse`;

  const contents = turns.map((t) => {
    const parts: unknown[] = [];
    if (t.role === "user" && t.imageBase64 && config.supportsVision) {
      parts.push({ inlineData: { mimeType: MEDIA_TYPE, data: t.imageBase64 } });
    }
    const text = t.role === "user" && t.ocrText ? `[Image OCR text]:\n${t.ocrText}\n\n${t.text}` : t.text;
    parts.push({ text });
    return { role: t.role === "assistant" ? "model" : "user", parts };
  });

  return {
    url,
    headers: { "Content-Type": "application/json", "x-goog-api-key": config.apiKey },
    body: { contents },
  };
}

// ---------- token extraction per provider ----------

function extractToken(provider: Provider, json: any): string | null {
  switch (provider) {
    case "anthropic":
      return json.type === "content_block_delta" ? json.delta?.text ?? null : null;
    case "gemini":
      return json.candidates?.[0]?.content?.parts?.[0]?.text ?? null;
    default:
      return json.choices?.[0]?.delta?.content ?? null;
  }
}

function buildRequest(config: ApiConfig, turns: ChatTurn[]): Request {
  switch (config.provider) {
    case "anthropic":
      return anthropicRequest(config, turns);
    case "gemini":
      return geminiRequest(config, turns);
    default:
      return openaiRequest(config, turns);
  }
}

/** Pull the available model ids from the provider's models endpoint. */
export async function fetchModels(config: ApiConfig): Promise<string[]> {
  let url: string;
  let headers: Record<string, string>;

  if (config.provider === "anthropic") {
    const base = trimSlash(withScheme(config.baseUrl)) || "https://api.anthropic.com";
    url = `${base.replace(/\/v1\/messages$/i, "")}/v1/models`;
    headers = {
      "x-api-key": config.apiKey,
      "anthropic-version": config.apiVersion?.trim() || "2023-06-01",
      "anthropic-dangerous-direct-browser-access": "true",
    };
  } else if (config.provider === "gemini") {
    const base = trimSlash(withScheme(config.baseUrl)) || "https://generativelanguage.googleapis.com";
    url = `${base}/v1beta/models`;
    headers = { "x-goog-api-key": config.apiKey };
  } else {
    url = openaiUrl(config).replace(/\/chat\/completions(\?.*)?$/i, "/models");
    headers = openaiHeaders(config);
  }

  const res = await fetch(url, { headers });
  if (!res.ok) throw new Error(`${res.status}: ${await res.text()}`);
  const json: any = await res.json();

  let ids: string[];
  if (config.provider === "gemini") {
    ids = (json.models ?? [])
      .filter((m: any) => !m.supportedGenerationMethods || m.supportedGenerationMethods.includes("generateContent"))
      .map((m: any) => String(m.name ?? "").replace(/^models\//, ""));
  } else {
    // OpenAI + Anthropic both return { data: [{ id }] }
    ids = (json.data ?? json.models ?? []).map((m: any) => m.id ?? m.name ?? m);
  }
  return [...new Set(ids.filter(Boolean))].sort();
}

export async function* streamChat(
  config: ApiConfig,
  turns: ChatTurn[],
  signal?: AbortSignal,
): AsyncGenerator<string> {
  const { url, headers, body } = buildRequest(config, turns);

  let response: Response;
  try {
    response = await fetch(url, {
      method: "POST",
      headers,
      body: JSON.stringify(body),
      signal,
    });
  } catch (e: any) {
    if (e?.name === "AbortError") throw e;
    throw new Error(`连接失败：${e?.message || e}\n接口：${url}\n（检查接口地址 / API Key / 网络）`);
  }

  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(`接口返回 ${response.status}：${errorText}\n接口：${url}`);
  }

  const reader = response.body?.getReader();
  if (!reader) throw new Error("No response body");

  const decoder = new TextDecoder();
  let buffer = "";

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;

    buffer += decoder.decode(value, { stream: true });
    const lines = buffer.split("\n");
    buffer = lines.pop() || "";

    for (const line of lines) {
      const trimmed = line.trim();
      if (!trimmed || trimmed === "data: [DONE]") continue;
      if (!trimmed.startsWith("data: ")) continue; // skips SSE `event:` lines too

      try {
        const json = JSON.parse(trimmed.slice(6));
        const token = extractToken(config.provider, json);
        if (token) yield token;
      } catch {
        // skip malformed lines
      }
    }
  }
}
