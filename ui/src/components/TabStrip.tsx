import { For, Show } from "solid-js";
import { closeRepoTab, openInNewWindow, openRepo, pickNewTab, store } from "../store";
import { openContextMenu } from "./ContextMenu";
import { CloseIcon, PlusIcon } from "./Icons";

const tabName = (path: string) => store.repositories.find((r) => r.path === path)?.name ?? path.split(/[\\/]/).pop();

export function TabStrip() {
  return (
    <div class="tab-strip" classList={{ shown: store.tabs.length > 1 }}>
      <Show when={store.tabs.length > 1}>
        <For each={store.tabs}>
          {(path) => (
            <div
              class="repo-tab"
              classList={{ active: store.repo?.id === path || (!store.repo && store.opening === path) }}
              title={path}
              onClick={() => store.repo?.id !== path && openRepo(path)}
              onAuxClick={(e) => e.button === 1 && closeRepoTab(path)}
              onContextMenu={(e) =>
                openContextMenu(e, [
                  {
                    label: "Move to new window",
                    run: () => {
                      openInNewWindow(path);
                      closeRepoTab(path);
                    },
                  },
                  { label: "Close other tabs", run: () => store.tabs.filter((t) => t !== path).forEach(closeRepoTab) },
                  "separator",
                  { label: "Close tab", run: () => closeRepoTab(path) },
                ])
              }
            >
              <span class="grow">{tabName(path)}</span>
              <button
                class="tab-close"
                title="Close tab (Ctrl+W)"
                onClick={(e) => {
                  e.stopPropagation();
                  closeRepoTab(path);
                }}
              >
                <CloseIcon />
              </button>
            </div>
          )}
        </For>
        <button class="tab-new" title="Open a repository in a new tab (Ctrl+Alt+T)" onClick={pickNewTab}>
          <PlusIcon />
        </button>
      </Show>
    </div>
  );
}
