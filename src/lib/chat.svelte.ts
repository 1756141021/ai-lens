import { streamChat, type ChatTurn, type ApiConfig } from "./api";

export interface ChatMessage {
  role: "user" | "assistant";
  content: string;
  imageBase64?: string;
  ocrText?: string;
}

let messages = $state<ChatMessage[]>([]);
let streaming = $state(false);
let error = $state<string | null>(null);
// Transient "正在搜索：…" line while the model uses web tools; gone once text flows.
let toolStatus = $state<string | null>(null);
let abortController: AbortController | null = null;

export function getMessages() {
  return messages;
}

export function isStreaming() {
  return streaming;
}

export function getError() {
  return error;
}

export function getToolStatus() {
  return toolStatus;
}

export function clearChat() {
  messages = [];
  error = null;
  cancelStream();
}

// Snapshot/restore so 新问题 can be undone before the new question is sent:
// we clear the conversation to look fresh, but can put it back if the user backs out.
export function snapshotMessages(): ChatMessage[] {
  return messages.map((m) => ({ ...m }));
}

export function restoreMessages(snap: ChatMessage[]) {
  cancelStream();
  messages = snap;
  error = null;
}

export function cancelStream() {
  if (abortController) {
    abortController.abort();
    abortController = null;
  }
  streaming = false;
}

export async function sendMessage(
  config: ApiConfig,
  text: string,
  imageBase64?: string,
  ocrText?: string,
) {
  error = null;

  // ocrText is stored on the turn itself so history stays self-contained.
  messages = [...messages, { role: "user", content: text, imageBase64, ocrText }];

  const turns: ChatTurn[] = messages.map((msg) => ({
    role: msg.role,
    text: msg.content,
    imageBase64: msg.imageBase64,
    ocrText: msg.ocrText,
  }));

  messages = [...messages, { role: "assistant", content: "" }];
  // The reactive proxy element — mutating its content is O(1) and reactive,
  // instead of copying the whole array on every streamed token.
  const assistantMsg = messages[messages.length - 1];

  streaming = true;
  const ctl = new AbortController();
  abortController = ctl;
  let stickyStatus = false;

  try {
    for await (const ev of streamChat(config, turns, ctl.signal)) {
      if (ev.type === "text") {
        assistantMsg.content += ev.text;
        if (!stickyStatus) toolStatus = null;
      } else {
        toolStatus = ev.text;
        stickyStatus = !!ev.sticky;
      }
    }
  } catch (e: any) {
    // Stop must end quietly — but the abort rejection isn't always named
    // AbortError (plugin-http's mid-stream cancellation isn't), so check the
    // signal, not the name.
    if (e.name !== "AbortError" && !ctl.signal.aborted) {
      error = e.message || "Unknown error";
    }
  } finally {
    streaming = false;
    toolStatus = null;
    abortController = null;
    // Drop the empty assistant placeholder if the stream produced nothing
    // (error or abort before the first token) — otherwise it renders as a
    // stuck typing bubble and pollutes later requests with an empty turn.
    if (assistantMsg.content === "") {
      messages = messages.filter((m) => m !== assistantMsg);
    }
  }
}
