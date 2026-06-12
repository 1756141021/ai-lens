// Local executors for the app-level tool loop (联网搜索的「应用内」模式).
// The model asks; we run the search / fetch here and hand text back.
// Failures return a message string instead of throwing — the model can read
// "搜索失败：…" and adapt, while a user Stop (AbortError) still propagates.
import { fetch } from "@tauri-apps/plugin-http";

const UA =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
const FETCH_TIMEOUT_MS = 15_000;
const MAX_PAGE_CHARS = 8_000;
const MAX_BODY_BYTES = 5_000_000;

// Literal-level SSRF guard: the model must not reach the machine or the LAN.
// (DNS-rebinding is out of scope — every action is shown in the UI, and this
// is a single-user desktop tool. Documented in DEV_NOTES.)
function blockedHost(hostname: string): boolean {
  const h = hostname.toLowerCase().replace(/^\[|\]$/g, "");
  if (h === "localhost" || h.endsWith(".localhost") || h.endsWith(".local")) return true;
  if (h === "::1" || h === "::" || h.startsWith("fe80:") || h.startsWith("fc") || h.startsWith("fd"))
    return true;
  const m = h.match(/^(\d+)\.(\d+)\.(\d+)\.(\d+)$/);
  if (m) {
    const a = Number(m[1]);
    const b = Number(m[2]);
    if (a === 0 || a === 10 || a === 127) return true;
    if (a === 172 && b >= 16 && b <= 31) return true;
    if (a === 192 && b === 168) return true;
    if (a === 169 && b === 254) return true;
  }
  return false;
}

/** Fetch with a hard timeout, telling a user Stop apart from the timer. */
async function guardedFetch(
  url: string,
  init: { method?: string; headers: Record<string, string>; body?: string },
  signal: AbortSignal | undefined,
): Promise<{ res?: Response; err?: string }> {
  const ctl = new AbortController();
  let timedOut = false;
  const timer = setTimeout(() => {
    timedOut = true;
    ctl.abort();
  }, FETCH_TIMEOUT_MS);
  const onAbort = () => ctl.abort();
  signal?.addEventListener("abort", onAbort, { once: true });
  try {
    const res = await fetch(url, { ...init, signal: ctl.signal });
    return { res };
  } catch (e: any) {
    if (signal?.aborted) throw e; // user pressed Stop — let the stream die
    if (timedOut) return { err: `超时（${FETCH_TIMEOUT_MS / 1000} 秒无响应）` };
    return { err: String(e?.message || e) };
  } finally {
    clearTimeout(timer);
    signal?.removeEventListener("abort", onAbort);
  }
}

function htmlToText(html: string): { title: string; text: string } {
  const doc = new DOMParser().parseFromString(html, "text/html");
  doc
    .querySelectorAll("script,style,noscript,svg,iframe,canvas,nav,header,footer,form,aside")
    .forEach((el) => el.remove());
  const root = doc.querySelector("article") ?? doc.querySelector("main") ?? doc.body;
  const text = (root?.textContent ?? "")
    .replace(/[ \t ]+/g, " ")
    .replace(/\s*\n\s*/g, "\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
  return { title: doc.title?.trim() ?? "", text };
}

export async function fetchUrl(rawUrl: string, signal?: AbortSignal): Promise<string> {
  let u: URL;
  try {
    u = new URL(rawUrl);
  } catch {
    return `读取失败：不是合法的网址（${rawUrl}）`;
  }
  if (u.protocol !== "http:" && u.protocol !== "https:") return "读取失败：只允许 http/https 网址";
  if (blockedHost(u.hostname)) return "读取失败：禁止访问本机或内网地址";

  const { res, err } = await guardedFetch(
    u.href,
    {
      headers: {
        "User-Agent": UA,
        Accept: "text/html,application/xhtml+xml;q=0.9,text/plain;q=0.8,application/json;q=0.7,*/*;q=0.5",
      },
    },
    signal,
  );
  if (!res) return `读取失败：${err}`;
  if (!res.ok) return `读取失败：网站返回 ${res.status}`;

  const len = Number(res.headers.get("content-length") || 0);
  if (len > MAX_BODY_BYTES) return "读取失败：页面太大";
  const ctype = (res.headers.get("content-type") || "").toLowerCase();
  if (!/text\/|html|json|xml/.test(ctype)) return `读取失败：不支持的内容类型（${ctype || "未知"}）`;

  const raw = await res.text();
  let title = "";
  let text: string;
  if (/html/.test(ctype)) {
    ({ title, text } = htmlToText(raw));
  } else {
    text = raw.trim();
  }
  if (!text) return "读取成功，但页面没有可提取的正文（可能是纯脚本渲染的页面）";
  const clipped =
    text.length > MAX_PAGE_CHARS ? text.slice(0, MAX_PAGE_CHARS) + "\n…（已截断）" : text;
  return `${title ? `【${title}】\n` : ""}${u.href}\n\n${clipped}`;
}

type SearchItem = { title: string; url: string; snippet: string };

function formatResults(items: SearchItem[]): string {
  if (!items.length) return "没有找到结果，换个关键词试试";
  return items
    .map((r, i) => `${i + 1}. ${r.title}\n   ${r.url}${r.snippet ? `\n   ${r.snippet}` : ""}`)
    .join("\n");
}

async function tavilySearch(query: string, key: string, signal?: AbortSignal): Promise<string> {
  const { res, err } = await guardedFetch(
    "https://api.tavily.com/search",
    {
      method: "POST",
      headers: { "Content-Type": "application/json", Authorization: `Bearer ${key}` },
      // api_key in the body keeps older Tavily deployments happy alongside Bearer auth
      body: JSON.stringify({ query, max_results: 6, api_key: key }),
    },
    signal,
  );
  if (!res) return `搜索失败：${err}`;
  if (!res.ok) return `搜索失败：Tavily 返回 ${res.status}（检查设置里的 Tavily Key）`;
  const json: any = await res.json();
  const items = (json.results ?? []).map((r: any) => ({
    title: String(r.title ?? ""),
    url: String(r.url ?? ""),
    snippet: String(r.content ?? "").slice(0, 200),
  }));
  return formatResults(items);
}

// Bing's RSS output: stable XML, no bot challenges — survives the
// proxy/datacenter-IP treatment that gets DDG's HTML endpoint 202'd.
async function bingSearch(
  query: string,
  signal?: AbortSignal,
): Promise<{ items?: SearchItem[]; err?: string }> {
  const { res, err } = await guardedFetch(
    "https://www.bing.com/search?format=rss&count=6&q=" + encodeURIComponent(query),
    { headers: { "User-Agent": UA } },
    signal,
  );
  if (!res) return { err };
  if (!res.ok) return { err: `必应返回 ${res.status}` };
  const doc = new DOMParser().parseFromString(await res.text(), "text/xml");
  const items = [...doc.querySelectorAll("item")].slice(0, 6).map((it) => ({
    title: (it.querySelector("title")?.textContent || "").trim(),
    url: (it.querySelector("link")?.textContent || "").trim(),
    snippet: (it.querySelector("description")?.textContent || "").trim().slice(0, 200),
  }));
  if (!items.length) return { err: "必应没有返回结果" };
  return { items };
}

async function ddgSearch(
  query: string,
  signal?: AbortSignal,
): Promise<{ items?: SearchItem[]; err?: string }> {
  const { res, err } = await guardedFetch(
    "https://html.duckduckgo.com/html/?q=" + encodeURIComponent(query),
    { headers: { "User-Agent": UA } },
    signal,
  );
  if (!res) return { err };
  if (!res.ok) return { err: `DuckDuckGo 返回 ${res.status}（可能被临时风控）` };

  const doc = new DOMParser().parseFromString(await res.text(), "text/html");
  const anchors = [...doc.querySelectorAll("a.result__a")];
  const snippets = [...doc.querySelectorAll(".result__snippet")];
  const items: SearchItem[] = [];
  for (let i = 0; i < anchors.length && items.length < 6; i++) {
    const href = anchors[i].getAttribute("href") || "";
    // Result links are redirect-wrapped: //duckduckgo.com/l/?uddg=<real-url>&rut=…
    let url = "";
    try {
      url = new URL(href, "https://duckduckgo.com").searchParams.get("uddg") || "";
    } catch {
      /* skip malformed */
    }
    if (!url || url.includes("duckduckgo.com/y.js")) continue; // ads / junk
    items.push({
      title: (anchors[i].textContent || "").trim(),
      url,
      snippet: (snippets[i]?.textContent || "").trim().slice(0, 200),
    });
  }
  if (!items.length) return { err: "DuckDuckGo 没有返回可解析的结果" };
  return { items };
}

export async function webSearch(
  query: string,
  tavilyKey: string,
  signal?: AbortSignal,
): Promise<string> {
  if (!query.trim()) return "搜索失败：查询词为空";
  if (tavilyKey) return tavilySearch(query, tavilyKey, signal);
  const bing = await bingSearch(query, signal);
  if (bing.items) return formatResults(bing.items);
  const ddg = await ddgSearch(query, signal);
  if (ddg.items) return formatResults(ddg.items);
  return `搜索失败：必应（${bing.err}）、DuckDuckGo（${ddg.err}）都没成功。稍后再试，或在设置里配一个 Tavily Key`;
}
