import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { createMemo, createResource, createSignal, For, Match, onCleanup, Show, Switch } from "solid-js";
import {
  api,
  type DesktopPreferences,
  type FetchAllRow,
  type GithubRepo,
  onFetchAllProgress,
  onSyncProgress,
  type RepoRecord,
  type RepoSettings,
  type Settings,
} from "../../api";
import { unwrap } from "solid-js/store";
import { cloneUrl, isShorthand, repoNameFromUrl } from "../../lib/clone";
import { fuzzyFilter } from "../../lib/fuzzy";
import { applyTheme, currentBranch, notify, openRepo, reloadRepositories, reportError, run, setStore, store } from "../../store";
import { Checkbox, Field, Select } from "../Controls";
import { Dialog } from "../Dialog";
import { Loading } from "../Loading";
import { RepoIcon } from "../Icons";
import { closeDialog, Tabs } from "./common";

const LICENSES = [
  { value: "", label: "None" },
  { value: "mit", label: "MIT License" },
  { value: "apache-2.0", label: "Apache License 2.0" },
  { value: "gpl-3.0", label: "GNU GPLv3" },
  { value: "bsd-3-clause", label: "BSD 3-Clause" },
  { value: "mpl-2.0", label: "Mozilla Public License 2.0" },
  { value: "unlicense", label: "The Unlicense" },
];

const sep = (path: string) => (path.includes("\\") ? "\\" : "/");

async function chooseFolder(title: string): Promise<string | null> {
  const dir = await openDialog({ directory: true, title });
  return typeof dir === "string" ? dir : null;
}

export function InitRepoDialog(p: { path: string }) {
  const create = async () => {
    const record = await run("Create repository", () => api.initRepository(p.path));
    if (!record) return;
    await reloadRepositories();
    await openRepo(record.path);
    closeDialog();
  };
  return (
    <Dialog
      title="Not a Git repository"
      onClose={closeDialog}
      footer={
        <>
          <button class="btn" onClick={closeDialog}>
            Cancel
          </button>
          <button class="btn primary" onClick={create}>
            Create a repository here
          </button>
        </>
      }
    >
      <p style={{ margin: 0 }}>
        <code>{p.path}</code> is not a Git repository. Would you like to create one there?
      </p>
    </Dialog>
  );
}

export function CreateRepoDialog() {
  const [name, setName] = createSignal("");
  const [description, setDescription] = createSignal("");
  const [parent, setParent] = createSignal("");
  const [readme, setReadme] = createSignal(true);
  const [gitignore, setGitignore] = createSignal("");
  const [license, setLicense] = createSignal("");
  const [templates] = createResource(
    () => !!store.account,
    (signedIn) => (signedIn ? api.gitignoreTemplates().catch(() => [] as string[]) : Promise.resolve([] as string[])),
  );
  const ignoreOptions = createMemo(() => [{ value: "", label: "None" }, ...(templates() ?? []).map((t) => ({ value: t, label: t }))]);
  const licenseOptions = () => (store.account ? LICENSES : LICENSES.slice(0, 2));

  const create = async () => {
    const record = await run("Create repository", () =>
      api.createRepository(parent(), name().trim(), description(), readme(), gitignore() || undefined, license() || undefined),
    );
    if (!record) return;
    await reloadRepositories();
    await openRepo(record.path);
    closeDialog();
  };

  return (
    <Dialog
      title="Create a new repository"
      onClose={closeDialog}
      footer={
        <>
          <button class="btn" onClick={closeDialog}>
            Cancel
          </button>
          <button class="btn primary" disabled={!name().trim() || !parent()} onClick={create}>
            Create repository
          </button>
        </>
      }
    >
      <Field label="Name">
        <input class="input" autofocus value={name()} onInput={(e) => setName(e.currentTarget.value)} />
      </Field>
      <Field label="Description">
        <input class="input" value={description()} onInput={(e) => setDescription(e.currentTarget.value)} />
      </Field>
      <Field label="Local path" hint={parent() && name() ? `Created at ${parent()}${sep(parent())}${name().trim()}` : undefined}>
        <div class="summary-row">
          <input class="input" value={parent()} onInput={(e) => setParent(e.currentTarget.value)} />
          <button class="btn" onClick={async () => setParent((await chooseFolder("Choose a location")) ?? parent())}>
            Choose…
          </button>
        </div>
      </Field>
      <Checkbox checked={readme()} onChange={setReadme} label="Initialize this repository with a README" />
      <Field label="Git ignore" hint={store.account ? undefined : "Sign in to GitHub.com to use .gitignore templates."}>
        <Select value={gitignore()} onChange={setGitignore} options={ignoreOptions()} disabled={!store.account} />
      </Field>
      <Field label="License">
        <Select value={license()} onChange={setLicense} options={licenseOptions()} />
      </Field>
    </Dialog>
  );
}

export function CloneDialog(p: { url?: string }) {
  const [tab, setTab] = createSignal<"github" | "url">(p.url || !store.account ? "url" : "github");
  const [url, setUrl] = createSignal(p.url ?? "");
  const [query, setQuery] = createSignal("");
  const [picked, setPicked] = createSignal<GithubRepo | null>(null);
  const [parent, setParent] = createSignal(localStorageGet("blink.cloneParent") ?? "");
  const [name, setName] = createSignal(p.url ? repoNameFromUrl(p.url) : "");
  const [shallow, setShallow] = createSignal(false);
  const [partial, setPartial] = createSignal(false);
  const [busy, setBusy] = createSignal(false);
  const [progress, setProgress] = createSignal<string>("");
  const [repos] = createResource(
    () => tab() === "github" && !!store.account,
    (load) => (load ? api.githubRepositories() : Promise.resolve([] as GithubRepo[])),
  );
  const filtered = createMemo(() => fuzzyFilter(repos() ?? [], query(), (r) => r.full_name));
  const source = () => (tab() === "github" ? picked()?.clone_url ?? "" : cloneUrl(url()));

  const unlisten = onSyncProgress((e) => {
    if (e.repo === "clone") setProgress(e.progress.percent != null ? `${e.progress.phase} ${e.progress.percent}%` : e.progress.phase);
  });
  onCleanup(() => unlisten.then((u) => u()));

  const clone = async () => {
    if (!source() || !parent() || !name().trim()) return;
    setBusy(true);
    localStorageSet("blink.cloneParent", parent());
    try {
      const record = await api.clone(source(), parent(), name().trim(), shallow() ? 1 : undefined, partial());
      await reloadRepositories();
      await openRepo(record.path);
      closeDialog();
      notify("success", `Cloned ${record.name}`);
    } catch (e) {
      reportError(e, "Clone failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title="Clone a repository"
      onClose={() => !busy() && closeDialog()}
      footer={
        <>
          <span class="muted small" style={{ "margin-right": "auto" }}>
            {busy() ? progress() || "Cloning…" : ""}
          </span>
          <button class="btn" disabled={busy()} onClick={closeDialog}>
            Cancel
          </button>
          <button class="btn primary" disabled={busy() || !source() || !parent() || !name().trim()} onClick={clone}>
            {busy() ? "Cloning…" : "Clone"}
          </button>
        </>
      }
    >
      <Tabs
        value={tab()}
        onChange={setTab}
        tabs={[
          ["github", "GitHub.com"],
          ["url", "URL"],
        ]}
      />
      <Switch>
        <Match when={tab() === "github" && !store.account}>
          <div class="empty">
            <div>
              <p>Sign in to see your repositories.</p>
              <button class="btn primary" onClick={() => setStore("dialog", { type: "signIn" })}>
                Sign in to GitHub.com
              </button>
            </div>
          </div>
        </Match>
        <Match when={tab() === "github"}>
          <input class="input" placeholder="Filter your repositories" value={query()} onInput={(e) => setQuery(e.currentTarget.value)} />
          <div class="pick-list">
            <Show when={!repos.loading} fallback={<div class="empty muted">Loading repositories…</div>}>
              <For each={filtered()} fallback={<div class="empty muted">No repositories</div>}>
                {(r) => (
                  <div
                    class="static-row"
                    classList={{ active: picked()?.id === r.id }}
                    onClick={() => {
                      setPicked(r);
                      setName(r.name);
                    }}
                  >
                    <RepoIcon />
                    <span class="grow">{r.full_name}</span>
                    <Show when={r.private}>
                      <span class="badge">private</span>
                    </Show>
                  </div>
                )}
              </For>
            </Show>
          </div>
        </Match>
        <Match when={tab() === "url"}>
          <Field label="Repository URL or GitHub owner/name" hint={isShorthand(url()) ? `Will clone ${cloneUrl(url())}` : undefined}>
            <input
              class="input"
              autofocus
              placeholder="https://github.com/owner/repo.git"
              value={url()}
              onInput={(e) => {
                setUrl(e.currentTarget.value);
                setName(repoNameFromUrl(e.currentTarget.value));
              }}
            />
          </Field>
        </Match>
      </Switch>
      <Field label="Local path" hint={parent() && name() ? `${parent()}${sep(parent())}${name()}` : undefined}>
        <div class="summary-row">
          <input class="input" placeholder="Parent folder" value={parent()} onInput={(e) => setParent(e.currentTarget.value)} />
          <button class="btn" onClick={async () => setParent((await chooseFolder("Clone into")) ?? parent())}>
            Choose…
          </button>
        </div>
      </Field>
      <Field label="Folder name">
        <input class="input" value={name()} onInput={(e) => setName(e.currentTarget.value)} />
      </Field>
      <Checkbox checked={shallow()} onChange={setShallow} label="Shallow clone (latest commit only)" />
      <Checkbox checked={partial()} onChange={setPartial} label="Partial clone (download file contents only when needed)" />
    </Dialog>
  );
}

function localStorageGet(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function localStorageSet(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {}
}

export function PublishDialog() {
  const [name, setName] = createSignal(store.repo?.name ?? "");
  const [description, setDescription] = createSignal("");
  const [isPrivate, setPrivate] = createSignal(true);
  const [org, setOrg] = createSignal("");
  const [orgs] = createResource(() => !!store.account, (yes) => (yes ? api.githubOrgs().catch(() => []) : Promise.resolve([])));
  const owners = createMemo(() => [
    { value: "", label: store.account?.login ?? "Personal account" },
    ...(orgs() ?? []).map((o) => ({ value: o.login, label: o.login })),
  ]);

  const publish = async () => {
    const repo = store.repo;
    if (!repo) return;
    const created = await run("Publish", () => api.publish(repo.id, name().trim(), description(), isPrivate(), org() || undefined));
    if (!created) return;
    closeDialog();
    notify("success", `Published to ${created.full_name}`);
    await reloadRepositories();
  };

  return (
    <Dialog
      title="Publish repository"
      onClose={closeDialog}
      footer={
        <>
          <button class="btn" onClick={closeDialog}>
            Cancel
          </button>
          <button class="btn primary" disabled={!name().trim() || !store.account} onClick={publish}>
            Publish repository
          </button>
        </>
      }
    >
      <Show
        when={store.account}
        fallback={
          <div>
            <p style={{ "margin-top": 0 }}>Sign in to GitHub.com to publish this repository.</p>
            <button class="btn primary" onClick={() => setStore("dialog", { type: "signIn" })}>
              Sign in
            </button>
          </div>
        }
      >
        <Field label="Name">
          <input class="input" autofocus value={name()} onInput={(e) => setName(e.currentTarget.value)} />
        </Field>
        <Field label="Description">
          <input class="input" value={description()} onInput={(e) => setDescription(e.currentTarget.value)} />
        </Field>
        <Field label="Owner">
          <Select value={org()} onChange={setOrg} options={owners()} />
        </Field>
        <Checkbox checked={isPrivate()} onChange={setPrivate} label="Keep this code private" />
        <span class="muted small">
          {currentBranch() ? `${currentBranch()} will be pushed and set as the upstream branch.` : "No branch is checked out."}
        </span>
      </Show>
    </Dialog>
  );
}

export function RepoSettingsDialog() {
  const [tab, setTab] = createSignal<"remote" | "ignore" | "config" | "sparse" | "fork">("remote");
  const [sparseText, setSparseText] = createSignal<string | null>(null);
  const [settings, setSettings] = createSignal<RepoSettings | null>(null);
  const [fork] = createResource(
    () => (store.repo && store.account ? store.repo.id : null),
    (id) => api.forkInfo(id).catch(() => null),
  );
  api
    .repoSettings(store.repo!.id)
    .then((s) => {
      setSettings(s);
      setSparseText(s.sparse ? s.sparse.join("\n") : null);
    })
    .catch(reportError);

  const update = (fn: (s: RepoSettings) => RepoSettings) => setSettings((s) => (s ? fn(structuredClone(s)) : s));
  const save = async () => {
    const current = settings();
    if (!current || !store.repo) return;
    const text = sparseText();
    const sparse = text === null ? null : text.split("\n").map((l) => l.trim()).filter(Boolean);
    const s = { ...current, sparse };
    const ok = await run("Save repository settings", () => api.saveRepoSettings(store.repo!.id, s).then(() => true));
    if (ok) closeDialog();
  };

  return (
    <Dialog
      title="Repository settings"
      onClose={closeDialog}
      wide
      footer={
        <>
          <button class="btn" onClick={closeDialog}>
            Cancel
          </button>
          <button class="btn primary" disabled={!settings()} onClick={save}>
            Save
          </button>
        </>
      }
    >
      <Tabs
        value={tab()}
        onChange={setTab}
        tabs={[
          ["remote", "Remote"],
          ["ignore", "Ignored files"],
          ["config", "Git config"],
          ["sparse", "Sparse checkout"],
          ...(fork()?.isFork ? ([["fork", "Fork behavior"]] as ["fork", string][]) : []),
        ]}
      />
      <Show when={settings()} fallback={<Loading />}>
        {(s) => (
          <Switch>
            <Match when={tab() === "remote"}>
              <For each={s().remotes}>
                {([name, url], i) => (
                  <Field label={name}>
                    <div class="summary-row">
                      <input
                        class="input"
                        value={url}
                        onInput={(e) => {
                          const v = e.currentTarget.value;
                          update((x) => ((x.remotes[i()][1] = v), x));
                        }}
                      />
                      <button class="btn danger" onClick={() => update((x) => ((x.remotes = x.remotes.filter((_, j) => j !== i())), x))}>
                        Remove
                      </button>
                    </div>
                  </Field>
                )}
              </For>
              <div>
                <button
                  class="btn"
                  onClick={() => {
                    const name = s().remotes.some(([n]) => n === "origin") ? `remote${s().remotes.length}` : "origin";
                    update((x) => ((x.remotes = [...x.remotes, [name, ""]]), x));
                  }}
                >
                  Add remote
                </button>
              </div>
            </Match>
            <Match when={tab() === "ignore"}>
              <span class="muted small">Edit .gitignore. Files matching these patterns are not shown in Changes.</span>
              <textarea
                class="textarea code-area"
                spellcheck={false}
                value={s().gitignore}
                onInput={(e) => {
                  const v = e.currentTarget.value;
                  update((x) => ((x.gitignore = v), x));
                }}
              />
            </Match>
            <Match when={tab() === "config"}>
              <span class="muted small">Leave blank to use your global Git identity.</span>
              <Field label="Name">
                <input class="input" value={s().userName ?? ""} onInput={(e) => { const v = e.currentTarget.value; update((x) => ((x.userName = v || null), x)); }} />
              </Field>
              <Field label="Email">
                <input class="input" value={s().userEmail ?? ""} onInput={(e) => { const v = e.currentTarget.value; update((x) => ((x.userEmail = v || null), x)); }} />
              </Field>
            </Match>
            <Match when={tab() === "sparse"}>
              <Checkbox
                checked={sparseText() !== null}
                onChange={(on) => setSparseText(on ? (sparseText() ?? "") : null)}
                label="Only check out some folders"
              />
              <Show when={sparseText() !== null}>
                <span class="muted small">One folder per line, relative to the repository root. Files in the root are always checked out.</span>
                <textarea
                  class="textarea code-area"
                  spellcheck={false}
                  placeholder={"src\ndocs"}
                  value={sparseText() ?? ""}
                  onInput={(e) => setSparseText(e.currentTarget.value)}
                />
              </Show>
            </Match>
            <Match when={tab() === "fork"}>
              <p style={{ margin: 0 }}>
                This repository is a fork of <strong>{fork()?.parent}</strong>.
              </p>
              <Show
                when={!fork()?.hasUpstream}
                fallback={<span class="muted">The parent repository is configured as the upstream remote. Pull requests target {fork()?.parent}.</span>}
              >
                <span class="muted">Add the parent as the upstream remote to contribute back and see its pull requests.</span>
                <div>
                  <button
                    class="btn primary"
                    onClick={async () => {
                      await run("Add upstream", () => api.addUpstream(store.repo!.id, fork()!.parentUrl!));
                      closeDialog();
                    }}
                  >
                    Use for contributing to {fork()?.parent}
                  </button>
                </div>
              </Show>
            </Match>
          </Switch>
        )}
      </Show>
    </Dialog>
  );
}

export function ImportDialog() {
  const [root, setRoot] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [fromDesktop, setFromDesktop] = createSignal<RepoRecord[] | null>(null);
  const [picked, setPicked] = createSignal<Set<string>>(new Set());
  const [prefItems, setPrefItems] = createSignal<DesktopPrefItem[]>([]);
  const [pickedPrefs, setPickedPrefs] = createSignal<Set<string>>(new Set());
  const findDesktop = async () => {
    setBusy(true);
    try {
      const [found, prefs, tools] = await Promise.all([
        api.desktopRepositories(),
        api.desktopPreferences().catch(() => null),
        api.detectIntegrations().catch(() => ({ editors: [], shells: [] })),
      ]);
      const items = prefs ? desktopPrefItems(prefs, tools) : [];
      setPrefItems(items);
      setPickedPrefs(new Set(items.map((i) => i.key)));
      setFromDesktop(found);
      setPicked(new Set(found.map((r) => r.path)));
    } catch (e) {
      reportError(e, "Could not read GitHub Desktop's repository list");
    } finally {
      setBusy(false);
    }
  };
  const togglePicked = (path: string, on: boolean) =>
    setPicked((prev) => {
      const next = new Set(prev);
      if (on) next.add(path);
      else next.delete(path);
      return next;
    });
  const addPicked = async () => {
    setBusy(true);
    const paths = [...picked()];
    let added = 0;
    for (const path of paths) {
      try {
        await api.addRepository(path);
        added++;
      } catch (e) {
        reportError(e, `Could not add ${path}`);
      }
    }
    await reloadRepositories();
    const prefs = prefItems().filter((i) => pickedPrefs().has(i.key));
    if (prefs.length && store.settings) {
      const next = structuredClone(unwrap(store.settings));
      for (const item of prefs) item.apply(next);
      await run("Import settings", () => api.saveSettings(next).then(() => setStore("settings", next)));
      applyTheme();
    }
    const parts = [];
    if (added || !prefs.length) parts.push(`${added} repositor${added === 1 ? "y" : "ies"}`);
    if (prefs.length) parts.push(`${prefs.length} setting${prefs.length === 1 ? "" : "s"}`);
    notify("success", `Imported ${parts.join(" and ")} from GitHub Desktop`);
    if (paths[0] && !store.repo) await openRepo(paths[0]);
    setBusy(false);
    closeDialog();
  };
  const scan = async () => {
    setBusy(true);
    try {
      const added = await api.importRepositories(root());
      await reloadRepositories();
      notify("success", added.length ? `Added ${added.length} repositor${added.length === 1 ? "y" : "ies"}` : "No repositories found");
      if (added[0] && !store.repo) await openRepo(added[0].path);
      closeDialog();
    } catch (e) {
      reportError(e, "Import failed");
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog
      title="Import repositories"
      onClose={closeDialog}
      footer={
        <>
          <button class="btn" onClick={closeDialog}>
            Cancel
          </button>
          <Show
            when={fromDesktop()}
            fallback={
              <button class="btn primary" disabled={!root() || busy()} onClick={scan}>
                {busy() ? "Scanning…" : "Import"}
              </button>
            }
          >
            <button class="btn primary" disabled={!(picked().size + pickedPrefs().size) || busy()} onClick={addPicked}>
              {busy() ? "Importing…" : "Import selected"}
            </button>
          </Show>
        </>
      }
    >
      <Show
        when={fromDesktop()}
        fallback={
          <>
            <span class="muted">Pick a folder and Blink adds every Git repository inside it.</span>
            <div class="summary-row">
              <input class="input" value={root()} onInput={(e) => setRoot(e.currentTarget.value)} placeholder="Folder" />
              <button class="btn" onClick={async () => setRoot((await chooseFolder("Import from")) ?? root())}>
                Choose…
              </button>
            </div>
            <div class="summary-row">
              <span class="muted grow">Coming from GitHub Desktop? Blink can read its repository list.</span>
              <button class="btn" disabled={busy()} onClick={findDesktop}>
                From GitHub Desktop
              </button>
            </div>
          </>
        }
      >
        {(found) => (
          <>
            <Show when={found().length} fallback={<span class="muted">No GitHub Desktop repositories that aren't already in Blink.</span>}>
              <span class="muted">These are in GitHub Desktop but not in Blink yet.</span>
              <div class="import-list">
                <For each={found()}>
                  {(r) => (
                    <Checkbox
                      checked={picked().has(r.path)}
                      onChange={(on) => togglePicked(r.path, on)}
                      label={
                        <span class="import-item" title={r.path}>
                          <span>{r.owner ? `${r.owner}/${r.name}` : r.name}</span>
                          <span class="muted">{r.path}</span>
                        </span>
                      }
                    />
                  )}
                </For>
              </div>
            </Show>
            <Show when={prefItems().length}>
              <span class="muted">Settings</span>
              <div class="import-list">
                <For each={prefItems()}>
                  {(item) => (
                    <Checkbox
                      checked={pickedPrefs().has(item.key)}
                      onChange={(on) =>
                        setPickedPrefs((prev) => {
                          const next = new Set(prev);
                          if (on) next.add(item.key);
                          else next.delete(item.key);
                          return next;
                        })
                      }
                      label={item.label}
                    />
                  )}
                </For>
              </div>
            </Show>
          </>
        )}
      </Show>
    </Dialog>
  );
}

export function FetchAllDialog() {
  const [rows, setRows] = createSignal<Record<string, FetchAllRow>>({});
  const [running, setRunning] = createSignal(true);
  const unlisten = onFetchAllProgress((row) => setRows((r) => ({ ...r, [row.path]: row })));
  onCleanup(() => unlisten.then((u) => u()));
  api
    .fetchAll()
    .then((all) => setRows(Object.fromEntries(all.map((r) => [r.path, r]))))
    .catch((e) => reportError(e, "Fetch all failed"))
    .finally(() => setRunning(false));
  const done = () => Object.keys(rows()).length;
  const status = (row: FetchAllRow | undefined) => {
    if (!row) return running() ? "Fetching…" : "";
    if (row.error) return row.error.split("\n")[0];
    const parts = [];
    if (row.ahead) parts.push(`${row.ahead} ahead`);
    if (row.behind) parts.push(`${row.behind} behind`);
    if (row.changes) parts.push(`${row.changes} changed file${row.changes === 1 ? "" : "s"}`);
    if (!parts.length) parts.push(row.fetched ? "Up to date" : "No remote");
    return parts.join(", ");
  };
  return (
    <Dialog
      title="Fetch all repositories"
      onClose={closeDialog}
      wide
      footer={
        <>
          <span class="muted grow">
            {running() ? `Fetched ${done()} of ${store.repositories.length}` : `Fetched ${done()} repositor${done() === 1 ? "y" : "ies"}`}
          </span>
          <button class="btn primary" onClick={closeDialog}>
            Close
          </button>
        </>
      }
    >
      <div class="fetch-all">
        <For each={store.repositories}>
          {(repo) => {
            const row = () => rows()[repo.path];
            return (
              <div
                class="static-row"
                classList={{ failed: !!row()?.error, behind: !!row()?.behind }}
                title={row()?.error ?? repo.path}
                onClick={() => {
                  closeDialog();
                  openRepo(repo.path);
                }}
              >
                <RepoIcon />
                <span class="grow">{repo.owner ? `${repo.owner}/${repo.name}` : repo.name}</span>
                <span class="muted fetch-status">{status(row())}</span>
              </div>
            );
          }}
        </For>
      </div>
    </Dialog>
  );
}

interface DesktopPrefItem {
  key: string;
  label: string;
  apply: (settings: Settings) => void;
}

function desktopPrefItems(prefs: DesktopPreferences, tools: { editors: { id: string; name: string }[]; shells: { id: string; name: string }[] }): DesktopPrefItem[] {
  const items: DesktopPrefItem[] = [];
  const editor = tools.editors.find((t) => t.id === prefs.editor);
  if (editor) items.push({ key: "editor", label: `External editor: ${editor.name}`, apply: (s) => (s.integrations.editor = editor.id) });
  const shell = tools.shells.find((t) => t.id === prefs.shell);
  if (shell) items.push({ key: "shell", label: `Shell: ${shell.name}`, apply: (s) => (s.integrations.shell = shell.id) });
  const theme = prefs.theme;
  if (theme) items.push({ key: "theme", label: `Theme: ${theme[0].toUpperCase()}${theme.slice(1)}`, apply: (s) => (s.appearance.theme = theme) });
  const openssh = prefs.useSystemOpenssh;
  if (openssh !== null) {
    items.push({ key: "openssh", label: `${openssh ? "Use" : "Don't use"} system OpenSSH`, apply: (s) => (s.advanced.useSystemOpenssh = openssh) });
  }
  return items;
}
