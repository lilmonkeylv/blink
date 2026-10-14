import { For, Show } from "solid-js";
import { closeRepoTab, openInNewWindow, openRepo, pickNewTab, shortcut, store } from "../store";
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
              data-tooltip={path}
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
                aria-label="Close tab"
                data-tooltip="Close tab"
                data-shortcut={shortcut("tab.close")}
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
        <button
          class="tab-new"
          aria-label="New tab"
          data-tooltip="Open a repository in a new tab"
          data-shortcut={shortcut("tab.new")}
          onClick={pickNewTab}
        >
          <PlusIcon />
        </button>
      </Show>
    </div>
  );
}
