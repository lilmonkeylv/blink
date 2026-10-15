import { Show } from "solid-js";

function Part(p: { path: string }) {
  const cut = () => p.path.lastIndexOf("/") + 1;
  return (
    <span class="file-part">
      <span class="dir">{p.path.slice(0, cut())}</span>
      <span class="name">{p.path.slice(cut())}</span>
    </span>
  );
}

export function FilePath(p: { path: string; oldPath?: string | null }) {
  return (
    <span class="file-path">
      <Show when={p.oldPath}>
        {(old) => (
          <>
            <Part path={old()} />
            <span class="arrow">{"->"}</span>
          </>
        )}
      </Show>
      <Part path={p.path} />
    </span>
  );
}
