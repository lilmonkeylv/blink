import { createResource, Show } from "solid-js";
import { api } from "../api";

export function Avatar(p: { url: string | null | undefined; size?: number }) {
  const [src] = createResource(
    () => p.url || null,
    (url) => api.avatar(url).catch(() => null),
  );
  const size = () => p.size ?? 40;
  return (
    <Show when={src()} fallback={<span class="avatar placeholder" style={{ width: `${size()}px`, height: `${size()}px` }} />}>
      {(s) => <img class="avatar" src={s()} width={size()} height={size()} alt="" />}
    </Show>
  );
}
