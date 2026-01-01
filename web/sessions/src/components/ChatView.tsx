import { For, Show, createSignal } from "solid-js";
import type { LogEntry, ContentBlock, Payload } from "../App";

interface Props {
  entries: LogEntry[];
}

export function ChatView(props: Props) {
  return (
    <div class="chat-view">
      <For each={props.entries}>{(entry) => <Entry entry={entry} />}</For>
    </div>
  );
}

function Entry(props: { entry: LogEntry }) {
  const entry = props.entry;
  const type = entry.type;

  // Claude Code format
  if (type === "user" || type === "assistant") {
    const content = entry.message?.content;
    if (!content) return null;
    return (
      <div class={`message message--${type}`}>
        <div class="message__role">{type}</div>
        <div class="message__content">
          <For each={content}>{(block) => <ContentBlockView block={block} />}</For>
        </div>
      </div>
    );
  }

  if (type === "summary") {
    return (
      <div class="message message--summary">
        <div class="message__role">Summary</div>
        <pre class="message__text">{entry.summary}</pre>
      </div>
    );
  }

  // Codex format
  if (type === "response_item" || type === "event_msg") {
    const payload = entry.payload;
    if (!payload) return null;
    return <PayloadView payload={payload} />;
  }

  return null;
}

function ContentBlockView(props: { block: ContentBlock }) {
  const block = props.block;

  if (block.type === "text") {
    return <div class="message__text" innerHTML={renderMarkdown(block.text || "")} />;
  }

  if (block.type === "tool_use") {
    return <ToolUse name={block.name || "unknown"} input={block.input} />;
  }

  if (block.type === "tool_result") {
    let text = "";
    if (typeof block.content === "string") {
      text = block.content;
    } else if (Array.isArray(block.content)) {
      text = block.content.map((c) => c.text || "").join("\n");
    }
    return <ToolResult text={text} isError={block.is_error} />;
  }

  return null;
}

function PayloadView(props: { payload: Payload }) {
  const p = props.payload;

  if (p.type === "message") {
    const role = p.role || "unknown";
    return (
      <div class={`message message--${role === "user" ? "user" : "assistant"}`}>
        <div class="message__role">{role}</div>
        <div class="message__content">
          <For each={p.content || []}>
            {(block) => (
              <Show when={block.text}>
                <div class="message__text" innerHTML={renderMarkdown(block.text!)} />
              </Show>
            )}
          </For>
        </div>
      </div>
    );
  }

  if (p.type === "function_call") {
    return <ToolUse name={p.name || ""} input={p.arguments} />;
  }

  if (p.type === "function_call_output") {
    return <ToolResult text={p.output || ""} />;
  }

  return null;
}

function ToolUse(props: { name: string; input: unknown }) {
  const [expanded, setExpanded] = createSignal(false);
  const inputStr =
    typeof props.input === "string" ? props.input : JSON.stringify(props.input, null, 2);

  return (
    <div class="tool-use">
      <button class="tool-use__header" onClick={() => setExpanded(!expanded())}>
        <span class="tool-use__name">{props.name}</span>
        <span class="tool-use__toggle">{expanded() ? "▼" : "▶"}</span>
      </button>
      <Show when={expanded()}>
        <pre class="tool-use__input">{inputStr}</pre>
      </Show>
    </div>
  );
}

function ToolResult(props: { text: string; isError?: boolean }) {
  const [expanded, setExpanded] = createSignal(false);
  const isLong = props.text.length > 500;

  return (
    <div class="tool-result" classList={{ "tool-result--error": props.isError }}>
      <Show when={isLong}>
        <button class="tool-result__toggle" onClick={() => setExpanded(!expanded())}>
          {expanded() ? "Collapse" : `Show output (${props.text.length} chars)`}
        </button>
      </Show>
      <Show when={!isLong || expanded()}>
        <pre class="tool-result__output">{props.text}</pre>
      </Show>
    </div>
  );
}

function renderMarkdown(text: string): string {
  // Escape HTML
  let html = text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

  // Code blocks
  html = html.replace(/```(\w*)\n([\s\S]*?)```/g, (_, lang, code) => {
    return `<pre class="code-block"><code>${code.trim()}</code></pre>`;
  });

  // Inline code
  html = html.replace(/`([^`]+)`/g, "<code>$1</code>");

  // Newlines
  html = html.replace(/\n/g, "<br>");

  return html;
}
