// Tauri's HTTP plugin routes requests through Rust, bypassing the webview's CORS
// (raw browser fetch to LLM APIs gets blocked → opaque "Failed to fetch").
import { fetch } from "@tauri-apps/plugin-http";
import { webSearch, fetchUrl } from "./webtools";

export type Provider = "openai" | "anthropic" | "gemini";

export interface ApiConfig {
  provider: Provider;
  baseUrl: string;
  apiKey: string;
  model: string;
  supportsVision: boolean;
  authHeader: string;
  apiVersion: string;
  /** Present only when 联网搜索 is enabled in Settings. */
  web?: { mode: "native" | "app"; tavilyKey: string };
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

// HTTP errors land in front of end users — lead with what to do,
// keep the raw status/body below for whoever needs to dig.
function humanizeHttpError(status: number, body: string, url: string): string {
  let advice: string | null = null;
  if (status === 401 || status === 403)
    advice = "API Key 无效或没有权限——去设置里检查 Key 是否填对、有没有过期";
  else if (status === 402) advice = "账户余额不足——去服务商控制台充值或查看额度";
  else if (status === 404) advice = "接口或模型不存在——检查接口地址，或点「拉取」重新选一个模型";
  else if (status === 400 || status === 422)
    advice = "请求被服务商拒绝——常见原因：模型不支持图片（去设置关掉「视觉识图」）、模型名拼错";
  else if (status === 408) advice = "请求超时——网络不稳或服务商响应慢，稍后再试";
  else if (status === 413) advice = "发送的内容太大——框小一点的区域试试";
  else if (status === 429) advice = "请求太频繁或额度用完了——稍等再试，或到服务商控制台看看余额";
  else if (status >= 500) advice = "服务商那边出错了——稍等片刻再试";
  const detail = body.length > 400 ? body.slice(0, 400) + "…" : body;
  return advice
    ? `${advice}\n详情：${status} ${detail}\n接口：${url}`
    : `接口返回 ${status}：${detail}\n接口：${url}`;
}

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
  const body: Record<string, unknown> = { model: config.model, messages, stream: true };
  if (config.web?.mode === "native") {
    // Only OpenRouter and Azure still reach this in native mode (wireFor sends
    // everything else to the Responses API). OpenRouter has a first-class
    // server tool; Azure gets web_search_options as a best effort.
    if (/openrouter/i.test(config.baseUrl)) body.tools = [{ type: "openrouter:web_search" }];
    else body.web_search_options = {};
  }
  return { url: openaiUrl(config), headers: openaiHeaders(config), body };
}

// ---------- OpenAI Responses API (native web search for OAI-compatible) ----------

// Chat completions has no general web_search tool — only the -search-preview
// models honor web_search_options. The Responses API does have one (it's the
// wire Codex speaks), and relays that pass it through get real provider-side
// search. Used only when 联网=native on the openai provider.
function responsesUrl(config: ApiConfig): string {
  let url = withScheme(trimSlash(config.baseUrl)) || "https://api.openai.com/v1";
  url = trimSlash(url);
  if (!/\/responses$/i.test(url)) {
    try {
      const u = new URL(url);
      if (u.pathname === "" || u.pathname === "/") url = `${u.origin}/v1/responses`;
      else url = url + "/responses";
    } catch {
      url = url + "/responses";
    }
  }
  return url;
}

function responsesRequest(config: ApiConfig, turns: ChatTurn[]): Request {
  const input = turns.map((t) => {
    if (t.role === "assistant") {
      return { role: "assistant", content: [{ type: "output_text", text: t.text }] };
    }
    const content: unknown[] = [];
    const hasImg = !!t.imageBase64 && config.supportsVision;
    if (hasImg) {
      content.push({ type: "input_image", image_url: `data:${MEDIA_TYPE};base64,${t.imageBase64}` });
    }
    const text = t.ocrText ? `[Image OCR text]:\n${t.ocrText}\n\n${t.text}` : t.text;
    content.push({ type: "input_text", text: hasImg ? text || "What do you see in this image?" : text });
    return { role: "user", content };
  });
  return {
    url: responsesUrl(config),
    headers: openaiHeaders(config),
    // store:false — the Responses API keeps conversations server-side by
    // default; chat completions never did, so keep that privacy semantic.
    body: { model: config.model, input, stream: true, store: false, tools: [{ type: "web_search" }] },
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

  const body: Record<string, unknown> = {
    model: config.model,
    max_tokens: 4096,
    stream: true,
    messages,
  };
  if (config.web?.mode === "native") {
    // Server-side tool: Anthropic runs the searches, results stream back inline.
    body.tools = [{ type: "web_search_20250305", name: "web_search", max_uses: 5 }];
  }

  return {
    url,
    headers: {
      "Content-Type": "application/json",
      "x-api-key": config.apiKey,
      "anthropic-version": config.apiVersion?.trim() || "2023-06-01",
      // webview origin is a browser context; Anthropic blocks it without this.
      "anthropic-dangerous-direct-browser-access": "true",
    },
    body,
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

  const body: Record<string, unknown> = { contents };
  // Google Search grounding — Gemini searches server-side, text streams as usual.
  if (config.web?.mode === "native") body.tools = [{ google_search: {} }];

  return {
    url,
    headers: { "Content-Type": "application/json", "x-goog-api-key": config.apiKey },
    body,
  };
}

// ---------- streaming events ----------

/** What streamChat yields: answer text, or a status line ("正在搜索：…") so
 *  the user always sees what the model is doing. Transient statuses clear as
 *  soon as text flows; sticky ones stay until the stream ends (Gemini sends
 *  its searched-queries metadata alongside the text, not before it). */
export type StreamEvent =
  | { type: "text"; text: string }
  | { type: "status"; text: string; sticky?: boolean };

/** The actual protocol a request is spoken in — usually the provider itself,
 *  but native web search upgrades plain OpenAI endpoints to the Responses API. */
type Wire = Provider | "openai-responses";

function wireFor(config: ApiConfig): Wire {
  if (
    config.provider === "openai" &&
    config.web?.mode === "native" &&
    !/openrouter/i.test(config.baseUrl) &&
    !config.baseUrl.includes(".openai.azure.com")
  ) {
    return "openai-responses";
  }
  return config.provider;
}

function extractEvents(wire: Wire, json: any): StreamEvent[] {
  switch (wire) {
    case "anthropic": {
      if (json.type === "content_block_delta") {
        const t = json.delta?.text;
        return t ? [{ type: "text", text: t }] : [];
      }
      // Server-side web search shows up as extra content blocks around the text.
      if (json.type === "content_block_start") {
        const bt = json.content_block?.type;
        if (bt === "server_tool_use") return [{ type: "status", text: "正在搜索网页…" }];
        if (bt === "web_search_tool_result")
          return [{ type: "status", text: "已拿到搜索结果，正在阅读…" }];
      }
      return [];
    }
    case "gemini": {
      const out: StreamEvent[] = [];
      const cand = json.candidates?.[0];
      const queries = cand?.groundingMetadata?.webSearchQueries;
      if (Array.isArray(queries) && queries.length)
        out.push({ type: "status", text: `已搜索：${queries.join("、")}`, sticky: true });
      const t = cand?.content?.parts?.[0]?.text;
      if (t) out.push({ type: "text", text: t });
      return out;
    }
    case "openai-responses": {
      const t = json.type;
      if (t === "response.output_text.delta" && json.delta)
        return [{ type: "text", text: String(json.delta) }];
      // different implementations emit different progress events — catch both
      if (
        (t === "response.output_item.added" && json.item?.type === "web_search_call") ||
        t === "response.web_search_call.in_progress" ||
        t === "response.web_search_call.searching"
      ) {
        return [{ type: "status", text: "正在搜索网页…" }];
      }
      if (t === "error") throw new Error(json.message || "Responses API 流错误");
      return [];
    }
    default: {
      const t = json.choices?.[0]?.delta?.content;
      return t ? [{ type: "text", text: t }] : [];
    }
  }
}

function buildRequest(wire: Wire, config: ApiConfig, turns: ChatTurn[]): Request {
  switch (wire) {
    case "anthropic":
      return anthropicRequest(config, turns);
    case "gemini":
      return geminiRequest(config, turns);
    case "openai-responses":
      return responsesRequest(config, turns);
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
  if (!res.ok) throw new Error(humanizeHttpError(res.status, await res.text(), url));
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

async function doFetch(
  url: string,
  headers: Record<string, string>,
  body: unknown,
  signal?: AbortSignal,
): Promise<Response> {
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
    throw new Error(humanizeHttpError(response.status, await response.text(), url));
  }
  return response;
}

async function* readSse(response: Response): AsyncGenerator<any> {
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
        yield JSON.parse(trimmed.slice(6));
      } catch {
        // skip malformed lines
      }
    }
  }
}

// ---------- app-level tool loop (应用内联网, OpenAI-compatible only) ----------

const TOOL_DEFS = [
  {
    type: "function",
    function: {
      name: "web_search",
      description:
        "Search the web. Returns the top results as titles, URLs and snippets. Use fetch_url to read a result in full.",
      parameters: {
        type: "object",
        properties: { query: { type: "string", description: "The search query" } },
        required: ["query"],
      },
    },
  },
  {
    type: "function",
    function: {
      name: "fetch_url",
      description: "Fetch a public web page and return its readable text content.",
      parameters: {
        type: "object",
        properties: { url: { type: "string", description: "Absolute http(s) URL" } },
        required: ["url"],
      },
    },
  },
];

const MAX_TOOL_ROUNDS = 4;

/** Function-calling loop: stream → model requests a tool → run it locally →
 *  append the result → stream again. Tool exchanges live only inside this
 *  call; conversation history keeps just the final question/answer turns. */
async function* streamOpenaiTools(
  config: ApiConfig,
  turns: ChatTurn[],
  signal?: AbortSignal,
): AsyncGenerator<StreamEvent> {
  const base = openaiRequest(config, turns);
  const messages = (base.body as { messages: unknown[] }).messages;
  let wroteText = false;

  for (let round = 0; ; round++) {
    const final = round >= MAX_TOOL_ROUNDS; // budget spent — force an answer
    const body = {
      ...(base.body as object),
      messages,
      tools: TOOL_DEFS,
      tool_choice: final ? "none" : "auto",
    };

    const response = await doFetch(base.url, base.headers, body, signal);
    const calls: { id: string; name: string; args: string }[] = [];
    let text = "";

    for await (const json of readSse(response)) {
      const delta = json.choices?.[0]?.delta;
      if (!delta) continue;
      if (delta.content) {
        // Blank line between pre-tool commentary and the post-tool answer.
        if (wroteText && !text) yield { type: "text", text: "\n\n" };
        text += delta.content;
        wroteText = true;
        yield { type: "text", text: delta.content };
      }
      // tool_calls stream in fragments: id/name first, arguments split across
      // many deltas — accumulate by index.
      for (const tc of delta.tool_calls ?? []) {
        const slot = (calls[tc.index ?? 0] ??= { id: "", name: "", args: "" });
        if (tc.id) slot.id = tc.id;
        // name is atomic (sent once); only arguments stream in fragments. Some
        // relays re-send the name — assign, don't append, or it doubles up.
        if (tc.function?.name) slot.name = tc.function.name;
        if (tc.function?.arguments) slot.args += tc.function.arguments;
      }
    }

    const live = calls.filter(Boolean);
    if (!live.length || final) return;
    live.forEach((c, i) => {
      if (!c.id) c.id = `call_${round}_${i}`; // some relays omit ids
    });

    messages.push({
      role: "assistant",
      content: text || null,
      tool_calls: live.map((c) => ({
        id: c.id,
        type: "function",
        function: { name: c.name, arguments: c.args },
      })),
    });

    for (const c of live) {
      let args: any = {};
      try {
        args = JSON.parse(c.args || "{}");
      } catch {
        // leave args empty — the executor reports the bad input back to the model
      }
      let result: string;
      if (c.name === "web_search") {
        const q = String(args.query ?? "");
        yield { type: "status", text: `正在搜索：${q}` };
        result = await webSearch(q, config.web?.tavilyKey || "", signal);
      } else if (c.name === "fetch_url") {
        const raw = String(args.url ?? "");
        let host = raw;
        try {
          host = new URL(raw).hostname;
        } catch {
          // not a URL — fetchUrl will say so in its result
        }
        yield { type: "status", text: `正在读取：${host}` };
        result = await fetchUrl(raw, signal);
      } else {
        result = `未知工具：${c.name}`;
      }
      messages.push({ role: "tool", tool_call_id: c.id, content: result });
    }
    yield { type: "status", text: "正在整理…" };
  }
}

export async function* streamChat(
  config: ApiConfig,
  turns: ChatTurn[],
  signal?: AbortSignal,
): AsyncGenerator<StreamEvent> {
  // App-level web tools only exist for OpenAI-compatible endpoints; Anthropic
  // and Gemini users get the (better) provider-native search instead.
  if (config.provider === "openai" && config.web?.mode === "app") {
    yield* streamOpenaiTools(config, turns, signal);
    return;
  }

  const wire = wireFor(config);
  const { url, headers, body } = buildRequest(wire, config, turns);
  const response = await doFetch(url, headers, body, signal);
  for await (const json of readSse(response)) {
    yield* extractEvents(wire, json);
  }
}
