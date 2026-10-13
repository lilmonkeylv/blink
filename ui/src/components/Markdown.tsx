import { createMemo, For, Match, Switch } from "solid-js";
import { Dynamic } from "solid-js/web";
import { api } from "../api";
import { parseMarkdown, type Inline } from "../lib/markdown";
import { reportError } from "../store";

function Inlines(p: { nodes: Inline[] }) {
  return (
    <For each={p.nodes}>
      {(node) => (
        <Switch>
          <Match when={node.type === "text" && node}>{(n) => n().text}</Match>
          <Match when={node.type === "code" && node}>{(n) => <code>{n().text}</code>}</Match>
          <Match when={node.type === "strong" && node}>
            {(n) => (
              <strong>
                <Inlines nodes={n().children} />
              </strong>
            )}
          </Match>
          <Match when={node.type === "link" && node}>
            {(n) => (
              <a title={n().href} onClick={() => api.openExternal(n().href).catch(reportError)}>
                <Inlines nodes={n().children} />
              </a>
            )}
          </Match>
        </Switch>
      )}
    </For>
  );
}

export function Markdown(p: { source: string }) {
  const blocks = createMemo(() => parseMarkdown(p.source));
  return (
    <div class="markdown">
      <For each={blocks()}>
        {(block) => (
          <Switch>
            <Match when={block.type === "heading" && block}>
              {(b) => (
                <Dynamic component={`h${Math.min(6, b().level + 1)}`}>
                  <Inlines nodes={b().children} />
                </Dynamic>
              )}
            </Match>
            <Match when={block.type === "list" && block}>
              {(b) => (
                <ul>
                  <For each={b().items}>
                    {(item) => (
                      <li>
                        <Inlines nodes={item} />
                      </li>
                    )}
                  </For>
                </ul>
              )}
            </Match>
            <Match when={block.type === "paragraph" && block}>
              {(b) => (
                <p>
                  <Inlines nodes={b().children} />
                </p>
              )}
            </Match>
          </Switch>
        )}
      </For>
    </div>
  );
}
