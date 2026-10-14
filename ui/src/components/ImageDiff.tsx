import { createEffect, createResource, createSignal, Match, Show, Switch } from "solid-js";
import { api, type ImagePair } from "../api";
import { store } from "../store";
import { Segmented } from "./Controls";

const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "avif"];

export function isImagePath(path: string): boolean {
  const ext = path.slice(path.lastIndexOf(".") + 1).toLowerCase();
  return path.includes(".") && IMAGE_EXTENSIONS.includes(ext);
}

type Mode = "two-up" | "swipe" | "onion" | "difference";

function size(bytes: number) {
  return bytes < 1024 ? `${bytes} B` : bytes < 1024 * 1024 ? `${(bytes / 1024).toFixed(1)} KB` : `${(bytes / 1048576).toFixed(1)} MB`;
}

function load(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = reject;
    img.src = src;
  });
}

function DifferenceCanvas(p: { pair: ImagePair }) {
  let canvas!: HTMLCanvasElement;
  createEffect(async () => {
    if (!p.pair.old || !p.pair.new) return;
    const [a, b] = await Promise.all([load(p.pair.old), load(p.pair.new)]);
    const w = Math.max(a.naturalWidth, b.naturalWidth);
    const h = Math.max(a.naturalHeight, b.naturalHeight);
    canvas.width = w;
    canvas.height = h;
    const ctx = canvas.getContext("2d")!;
    ctx.clearRect(0, 0, w, h);
    ctx.drawImage(a, 0, 0);
    ctx.globalCompositeOperation = "difference";
    ctx.drawImage(b, 0, 0);
    ctx.globalCompositeOperation = "source-over";
  });
  return <canvas ref={canvas} class="image-frame" />;
}

export function ImageDiff(p: { path: string; oldPath: string | null; sha?: string }) {
  const [mode, setMode] = createSignal<Mode>("two-up");
  const [amount, setAmount] = createSignal(50);
  const [pair] = createResource(
    () => (store.repo ? { id: store.repo.id, path: p.path, old: p.oldPath, sha: p.sha, gen: store.repo.generation } : null),
    ({ id, path, old, sha }) => api.imagePair(id, path, old, sha),
  );
  const both = () => !!pair()?.old && !!pair()?.new;

  return (
    <Show when={pair()} fallback={<div class="empty muted">Loading image…</div>}>
      {(img) => (
        <div class="image-diff">
          <Show when={both()}>
            <div class="image-toolbar">
              <Segmented
                value={mode()}
                onChange={setMode}
                options={[
                  { value: "two-up", label: "2-up" },
                  { value: "swipe", label: "Swipe" },
                  { value: "onion", label: "Onion skin" },
                  { value: "difference", label: "Difference" },
                ]}
              />
              <Show when={mode() === "swipe" || mode() === "onion"}>
                <input
                  type="range"
                  min="0"
                  max="100"
                  value={amount()}
                  aria-label={mode() === "swipe" ? "Swipe position" : "After image opacity"}
                  aria-valuetext={`${amount()}%`}
                  style={{ "--fill": `${amount()}%` }}
                  onInput={(e) => setAmount(Number(e.currentTarget.value))}
                />
              </Show>
            </div>
          </Show>
          <div class="image-stage">
            <Switch>
              <Match when={!both()}>
                <figure>
                  <img class="image-frame" src={(img().new ?? img().old)!} alt={p.path} />
                  <figcaption>
                    {img().new ? "Added" : "Deleted"} · {size(img().new ? img().newSize : img().oldSize)}
                  </figcaption>
                </figure>
              </Match>
              <Match when={mode() === "two-up"}>
                <figure>
                  <img class="image-frame deleted" src={img().old!} alt="Before" />
                  <figcaption>Before · {size(img().oldSize)}</figcaption>
                </figure>
                <figure>
                  <img class="image-frame added" src={img().new!} alt="After" />
                  <figcaption>After · {size(img().newSize)}</figcaption>
                </figure>
              </Match>
              <Match when={mode() === "swipe"}>
                <div class="image-stack">
                  <img class="image-frame" src={img().old!} alt="Before" />
                  <img class="image-frame overlay" src={img().new!} alt="After" style={{ "clip-path": `inset(0 ${100 - amount()}% 0 0)` }} />
                </div>
              </Match>
              <Match when={mode() === "onion"}>
                <div class="image-stack">
                  <img class="image-frame" src={img().old!} alt="Before" />
                  <img class="image-frame overlay" src={img().new!} alt="After" style={{ opacity: amount() / 100 }} />
                </div>
              </Match>
              <Match when={mode() === "difference"}>
                <DifferenceCanvas pair={img()} />
              </Match>
            </Switch>
          </div>
        </div>
      )}
    </Show>
  );
}
