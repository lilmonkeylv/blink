import { render } from "solid-js/web";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { App } from "./App";
import { addRepository } from "./store";
import "./styles.css";

if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
  const { installDevMock } = await import("./dev-mock");
  installDevMock();
}

render(() => <App />, document.getElementById("root")!);

try {
  getCurrentWebview()
    .onDragDropEvent((event) => {
      if (event.payload.type === "drop" && event.payload.paths[0]) addRepository(event.payload.paths[0]);
    })
    .catch(() => {});
} catch {}

window.addEventListener("contextmenu", (e) => {
  if (!(e.target as HTMLElement).closest("input, textarea, .code")) e.preventDefault();
});
