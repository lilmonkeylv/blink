import { createMemo, createResource, createSignal, For, Show } from "solid-js";
import { api, type Branch } from "../../api";
import { fuzzyFilter } from "../../lib/fuzzy";
import { currentBranch, setStore, store } from "../../store";
import { BranchIcon } from "../Icons";

export const closeDialog = () => setStore("dialog", null);

export function sanitizeBranchName(name: string): string {
  return name
    .trim()
    .replace(/\s+/g, "-")
    .replace(/[~^:?*[\\\x00-\x1f\x7f]+/g, "-")
    .replace(/\.{2,}/g, ".")
    .replace(/@\{/g, "-")
    .replace(/^[-./]+|[./]+$|\.lock$/g, "");
}

export function BranchPicker(p: {
  value: string | null;
  onChange: (name: string) => void;
  excludeCurrent?: boolean;
  localOnly?: boolean;
}) {
  const [query, setQuery] = createSignal("");
  const [branches] = createResource(() => store.repo?.id, (id) => api.branches(id));
  const list = createMemo(() =>
    fuzzyFilter(
      (branches() ?? []).filter(
        (b: Branch) => (!p.excludeCurrent || b.name !== currentBranch()) && (!p.localOnly || b.kind === "local"),
      ),
      query(),
      (b) => b.name,
    ),
  );
  return (
    <>
      <input class="input" placeholder="Filter branches" autofocus value={query()} onInput={(e) => setQuery(e.currentTarget.value)} />
      <div class="pick-list">
        <For each={list()} fallback={<div class="empty muted">No branches</div>}>
          {(b) => (
            <div class="static-row" classList={{ active: p.value === b.name }} onClick={() => p.onChange(b.name)}>
              <BranchIcon />
              <span class="grow">{b.name}</span>
              <Show when={b.kind === "remote"}>
                <span class="muted small">remote</span>
              </Show>
            </div>
          )}
        </For>
      </div>
    </>
  );
}

export function Tabs<T extends string>(p: { value: T; tabs: [T, string][]; onChange: (t: T) => void }) {
  return (
    <div class="tabs dialog-tabs" role="tablist">
      <For each={p.tabs}>
        {([id, label]) => (
          <button class="tab" role="tab" classList={{ active: p.value === id }} onClick={() => p.onChange(id)}>
            {label}
          </button>
        )}
      </For>
    </div>
  );
}
