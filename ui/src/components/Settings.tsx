import { createEffect, createMemo, createResource, createSignal, For, type JSX, Match, onCleanup, Show, Switch } from "solid-js";
import { createStore, unwrap } from "solid-js/store";
import { api, isCommandError, type Settings } from "../api";
import { ACCENT_PRESETS, normalizeHex, readableOn } from "../lib/appearance";
import { applyTheme, notify, reportError, saveSettings, setStore, store } from "../store";
import { Checkbox, Field, Select, type SelectOption } from "./Controls";
import { Dialog } from "./Dialog";
import { Avatar } from "./Avatar";
import { CheckIcon } from "./Icons";
import { SigningSetup } from "./SigningSetup";

const TABS = ["Accounts", "Integrations", "Git", "Appearance", "Notifications", "Prompts", "Advanced", "Accessibility"] as const;
type TabName = (typeof TABS)[number];
type Update = (fn: (s: Settings) => void) => void;

function Check(p: { label: string; checked: boolean; onChange: (v: boolean) => void; hint?: string }) {
  return (
    <div class="field">
      <Checkbox checked={p.checked} onChange={p.onChange} label={p.label} />
      <Show when={p.hint}>
        <span class="hint" style={{ "padding-left": "24px" }}>
          {p.hint}
        </span>
      </Show>
    </div>
  );
}

function Choice<T extends string>(p: { label: string; value: T; options: SelectOption<T>[]; onChange: (v: T) => void; hint?: string }) {
  return (
    <Field label={p.label} hint={p.hint}>
      <Select value={p.value} options={p.options} onChange={p.onChange} ariaLabel={p.label} />
    </Field>
  );
}

function Text(p: { label: string; value: string; onChange: (v: string) => void; placeholder?: string; type?: string; hint?: string }) {
  return (
    <Field label={p.label} hint={p.hint}>
      <input class="input" type={p.type ?? "text"} value={p.value} placeholder={p.placeholder} onInput={(e) => p.onChange(e.currentTarget.value)} />
    </Field>
  );
}

const THREAD_COUNTS = [1, 2, 4, 8, 16];

function Threads(p: { label: string; value: number | null; onChange: (v: number | null) => void; hint?: string }) {
  const options = () => {
    const counts = p.value && !THREAD_COUNTS.includes(p.value) ? [...THREAD_COUNTS, p.value].sort((a, b) => a - b) : THREAD_COUNTS;
    return [{ value: "", label: "Automatic" }, ...counts.map((n) => ({ value: String(n), label: String(n) }))];
  };
  return <Choice label={p.label} value={p.value ? String(p.value) : ""} options={options()} onChange={(v) => p.onChange(v ? Number(v) : null)} hint={p.hint} />;
}

function Pane(p: { children: JSX.Element }) {
  return <div class="pane">{p.children}</div>;
}

const HOURS = Array.from({ length: 24 }, (_, h) => ({ value: String(h), label: `${String(h).padStart(2, "0")}:00` }));

function Swatch(p: { color: string; label: string; checked: boolean; onSelect: () => void }) {
  return (
    <button
      type="button"
      role="radio"
      class="swatch"
      aria-checked={p.checked}
      aria-label={p.label}
      title={p.label}
      style={{ "--swatch": p.color, color: readableOn(p.color) }}
      onClick={p.onSelect}
    >
      <Show when={p.checked}>
        <CheckIcon />
      </Show>
    </button>
  );
}

function AccentPicker(p: { value: string | null; onChange: (v: string | null) => void }) {
  const current = () => normalizeHex(p.value);
  const isPreset = (hex: string | null) => !hex || ACCENT_PRESETS.some((c) => c.value === hex);
  const [custom, setCustom] = createSignal(isPreset(current()) ? "" : (current() ?? ""));
  const customHex = () => normalizeHex(custom());
  return (
    <Field label="Accent color" hint="Used for selection, focus rings and primary buttons.">
      <div class="swatches" role="radiogroup" aria-label="Accent color">
        <For each={ACCENT_PRESETS}>
          {(c, i) => (
            <Swatch
              color={c.value}
              label={i() === 0 ? `${c.label} (default)` : c.label}
              checked={i() === 0 ? !current() || current() === c.value : current() === c.value}
              onSelect={() => p.onChange(i() === 0 ? null : c.value)}
            />
          )}
        </For>
        <Swatch
          color={customHex() ?? "#8c959f"}
          label="Custom"
          checked={!isPreset(current())}
          onSelect={() => customHex() && p.onChange(customHex())}
        />
        <input
          class="input swatch-hex"
          placeholder="#rrggbb"
          aria-label="Custom accent color"
          spellcheck={false}
          value={custom()}
          classList={{ invalid: !!custom().trim() && !customHex() }}
          onInput={(e) => {
            setCustom(e.currentTarget.value);
            const hex = normalizeHex(e.currentTarget.value);
            if (hex) p.onChange(hex);
          }}
        />
      </div>
    </Field>
  );
}

function GitExecutableField(p: { value: string; onChange: (v: string) => void }) {
  const [status, setStatus] = createSignal<{ ok: boolean; text: string } | null>(null);
  const check = async () => {
    const path = p.value.trim();
    if (!path) return setStatus(null);
    try {
      setStatus({ ok: true, text: `Found Git ${await api.checkGitExecutable(path)}` });
    } catch (e) {
      setStatus({ ok: false, text: isCommandError(e) ? (e.detail ?? e.message) : String(e) });
    }
  };
  return (
    <Field label="Git executable" hint="Leave blank to use the Git found on PATH. Applies to the next Git command.">
      <div class="summary-row">
        <input
          class="input"
          placeholder="Found on PATH"
          spellcheck={false}
          value={p.value}
          onInput={(e) => {
            setStatus(null);
            p.onChange(e.currentTarget.value);
          }}
          onBlur={check}
        />
        <button class="btn" disabled={!p.value.trim()} onClick={check}>
          Test
        </button>
      </div>
      <Show when={status()}>{(st) => <span class={`hint ${st().ok ? "ok" : "error"}`}>{st().text}</span>}</Show>
    </Field>
  );
}

function EnterpriseAccounts() {
  const [accounts, { refetch }] = createResource(() => api.enterpriseAccounts());
  const [host, setHost] = createSignal("");
  const [token, setToken] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const add = async () => {
    setBusy(true);
    try {
      const added = await api.enterpriseSignIn(host(), token());
      notify("success", `Signed in to ${added.host} as ${added.account.login}`);
      setHost("");
      setToken("");
      refetch();
    } catch (e) {
      reportError(e, "Could not sign in");
    } finally {
      setBusy(false);
    }
  };
  const remove = async (h: string) => {
    await api.enterpriseSignOut(h).catch(reportError);
    refetch();
  };
  return (
    <>
      <h3 style={{ margin: "12px 0 0" }}>GitHub Enterprise</h3>
      <For each={accounts() ?? []}>
        {(a) => (
          <div class="summary-row">
            <div style={{ flex: 1 }}>
              <div style={{ "font-weight": 600 }}>{a.host}</div>
              <div class="muted">@{a.account.login}</div>
            </div>
            <button class="btn" onClick={() => remove(a.host)}>
              Sign out
            </button>
          </div>
        )}
      </For>
      <Field label="Server" hint="Your GitHub Enterprise Server address, or a ghe.com subdomain for Enterprise Cloud with data residency.">
        <input class="input" placeholder="github.example.com" value={host()} onInput={(e) => setHost(e.currentTarget.value)} />
      </Field>
      <Field label="Personal access token" hint="Create one on your server under Settings > Developer settings with the repo, read:org and workflow scopes. It's stored in your system keychain.">
        <input class="input" type="password" autocomplete="off" value={token()} onInput={(e) => setToken(e.currentTarget.value)} />
      </Field>
      <div>
        <button class="btn" disabled={busy() || !host().trim() || !token().trim()} onClick={add}>
          {busy() ? "Checking…" : "Add Enterprise account"}
        </button>
      </div>
    </>
  );
}

function AccountsTab() {
  const signOut = async () => {
    await api.signOut().catch(reportError);
    setStore({ account: null, checks: null });
    const revoke = await api.revokeUrl().catch(() => null);
    notify(
      "info",
      "Signed out of GitHub.com. Blink's token is deleted, but GitHub still lists the grant until you revoke it. Git keeps its own GitHub login for push and pull.",
      {
        sticky: true,
        action: revoke ? { label: "Revoke on GitHub", run: () => api.openExternal(revoke).catch(reportError) } : undefined,
      },
    );
  };
  return (
    <Pane>
      <h3 style={{ margin: 0 }}>GitHub.com</h3>
      <Show
        when={store.account}
        fallback={
          <>
            <span class="muted">Sign in to your GitHub.com account to access your repositories.</span>
            <div>
              <button class="btn primary" onClick={() => setStore("dialog", { type: "signIn" })}>
                Sign in to GitHub.com
              </button>
            </div>
          </>
        }
      >
        {(a) => (
          <div class="summary-row">
            <Avatar url={a().avatar_url} />
            <div style={{ flex: 1 }}>
              <div style={{ "font-weight": 600 }}>{a().name ?? a().login}</div>
              <div class="muted">@{a().login}</div>
            </div>
            <button class="btn" onClick={signOut}>
              Sign out
            </button>
          </div>
        )}
      </Show>
      <EnterpriseAccounts />
    </Pane>
  );
}

function IntegrationsTab(p: { s: Settings; set: Update }) {
  const [tools] = createResource(() => api.detectIntegrations());
  const editors = createMemo(() => [
    { value: "", label: tools.loading ? "Detecting…" : "First available" },
    ...(tools()?.editors ?? []).map((t) => ({ value: t.id, label: t.name })),
  ]);
  const shells = createMemo(() => [
    { value: "", label: tools.loading ? "Detecting…" : "System default" },
    ...(tools()?.shells ?? []).map((t) => ({ value: t.id, label: t.name })),
  ]);
  return (
    <Pane>
      <Choice label="External editor" value={p.s.integrations.editor ?? ""} options={editors()} onChange={(v) => p.set((x) => (x.integrations.editor = v || null))} />
      <Text
        label="Custom editor path"
        placeholder="Leave blank to use the editor above"
        value={p.s.integrations.customEditorPath ?? ""}
        onChange={(v) => p.set((x) => (x.integrations.customEditorPath = v || null))}
      />
      <Choice label="Shell" value={p.s.integrations.shell ?? ""} options={shells()} onChange={(v) => p.set((x) => (x.integrations.shell = v || null))} />
      <Text
        label="Custom shell path"
        placeholder="Leave blank to use the shell above"
        value={p.s.integrations.customShellPath ?? ""}
        onChange={(v) => p.set((x) => (x.integrations.customShellPath = v || null))}
      />
      <Choice
        label="Diff and merge tool"
        value={p.s.integrations.diffTool}
        options={[
          { value: "built-in", label: "Built-in" },
          { value: "vs-code", label: "Visual Studio Code" },
          { value: "beyond-compare", label: "Beyond Compare" },
          { value: "meld", label: "Meld" },
          { value: "k-diff3", label: "KDiff3" },
        ]}
        onChange={(v) => p.set((x) => (x.integrations.diffTool = v))}
      />
      <Check
        label="Open the terminal at the selected file's folder"
        checked={p.s.integrations.terminalOpensAtFile}
        onChange={(v) => p.set((x) => (x.integrations.terminalOpensAtFile = v))}
      />
    </Pane>
  );
}

function GitTab(p: { s: Settings; set: Update }) {
  const [identity] = createResource(() => api.gitIdentity());
  const [name, setName] = createSignal<string | null>(null);
  const [email, setEmail] = createSignal<string | null>(null);
  const saveIdentity = async () => {
    try {
      await api.setGitIdentity(name() ?? identity()?.name ?? "", email() ?? identity()?.email ?? "");
      notify("success", "Git identity saved");
    } catch (e) {
      reportError(e);
    }
  };
  return (
    <Pane>
      <Text label="Name" value={name() ?? identity()?.name ?? ""} onChange={setName} />
      <Text
        label="Email"
        value={email() ?? identity()?.email ?? ""}
        onChange={setEmail}
        hint={store.account ? "Use an email linked to your GitHub account so commits are attributed to you." : undefined}
      />
      <div>
        <button class="btn" onClick={saveIdentity}>
          Save identity
        </button>
      </div>
      <Text label="Default branch name for new repositories" value={p.s.git.defaultBranch} onChange={(v) => p.set((s) => (s.git.defaultBranch = v))} />
      <Choice
        label="Pull behavior"
        value={p.s.git.pullStrategy}
        options={[
          { value: "merge", label: "Merge" },
          { value: "rebase", label: "Rebase" },
          { value: "ff-only", label: "Fast-forward only" },
        ]}
        onChange={(v) => p.set((s) => (s.git.pullStrategy = v))}
      />
      <Check label="Prune remote-tracking branches on fetch" checked={p.s.git.pruneOnFetch} onChange={(v) => p.set((s) => (s.git.pruneOnFetch = v))} />
      <Check
        label="Sign commits"
        checked={p.s.git.signCommits}
        onChange={(v) => p.set((s) => (s.git.signCommits = v))}
        hint="Uses your Git signing configuration, such as gpg.format."
      />
      <Text
        label="Signing key"
        placeholder="Use user.signingkey from your Git config"
        value={p.s.git.signingKey ?? ""}
        onChange={(v) => p.set((s) => (s.git.signingKey = v.trim() ? v : null))}
        hint="A GPG key ID, or the path to an SSH key when gpg.format is ssh."
      />
      <GitExecutableField value={p.s.git.executablePath ?? ""} onChange={(v) => p.set((s) => (s.git.executablePath = v.trim() ? v : null))} />
      <SigningSetup />
    </Pane>
  );
}

export function SettingsDialog(p: { tab?: string }) {
  const [tab, setTab] = createSignal<TabName>((TABS.find((t) => t === p.tab) as TabName) ?? "Accounts");
  const [s, setS] = createStore<Settings>(structuredClone(unwrap(store.settings!)));
  const set: Update = (fn) =>
    setS((prev) => {
      const next = structuredClone(unwrap(prev));
      fn(next);
      return next;
    });
  createEffect(() => applyTheme(s));
  onCleanup(() => applyTheme());
  const close = () => setStore("dialog", null);
  const save = async () => {
    try {
      await saveSettings(structuredClone(unwrap(s)));
      close();
    } catch (e) {
      reportError(e, "Could not save settings");
    }
  };

  return (
    <Dialog
      title="Settings"
      onClose={close}
      wide
      footer={
        <>
          <button class="btn" onClick={close}>
            Cancel
          </button>
          <button class="btn primary" onClick={save}>
            Save
          </button>
        </>
      }
    >
      <div class="settings" style={{ margin: "-16px" }}>
        <nav>
          <For each={TABS}>
            {(t) => (
              <button classList={{ active: tab() === t }} onClick={() => setTab(t)}>
                {t}
              </button>
            )}
          </For>
        </nav>
        <Switch>
          <Match when={tab() === "Accounts"}>
            <AccountsTab />
          </Match>
          <Match when={tab() === "Integrations"}>
            <IntegrationsTab s={s} set={set} />
          </Match>
          <Match when={tab() === "Git"}>
            <GitTab s={s} set={set} />
          </Match>
          <Match when={tab() === "Appearance"}>
            <Pane>
              <Choice
                label="Theme"
                value={s.appearance.theme}
                options={[
                  { value: "system", label: "System" },
                  { value: "light", label: "Light" },
                  { value: "dark", label: "Dark" },
                ]}
                onChange={(v) => set((x) => (x.appearance.theme = v))}
              />
              <AccentPicker value={s.appearance.accentColor} onChange={(v) => set((x) => (x.appearance.accentColor = v))} />
              <Choice
                label="Density"
                value={s.appearance.density}
                options={[
                  { value: "comfortable", label: "Comfortable" },
                  { value: "compact", label: "Compact" },
                ]}
                onChange={(v) => set((x) => (x.appearance.density = v))}
              />
              <Choice
                label="Tab size"
                value={String(s.appearance.tabSize)}
                options={["1", "2", "3", "4", "5", "6", "8"].map((v) => ({ value: v, label: v }))}
                onChange={(v) => set((x) => (x.appearance.tabSize = Number(v)))}
              />
              <Text
                label="Diff font"
                placeholder="Cascadia Code"
                value={s.appearance.diffFont ?? ""}
                onChange={(v) => set((x) => (x.appearance.diffFont = v.trim() ? v : null))}
                hint="Any installed monospace font. Separate fallbacks with commas."
              />
              <Choice
                label="Diff font size"
                value={String(s.appearance.diffFontSize)}
                options={["10", "11", "12", "13", "14", "16", "18"].map((v) => ({ value: v, label: `${v} px` }))}
                onChange={(v) => set((x) => (x.appearance.diffFontSize = Number(v)))}
              />
              <Choice
                label="Dates"
                value={s.appearance.dateFormat}
                options={[
                  { value: "relative", label: "Relative (3 hours ago)" },
                  { value: "absolute", label: "Absolute" },
                ]}
                onChange={(v) => set((x) => (x.appearance.dateFormat = v))}
              />
            </Pane>
          </Match>
          <Match when={tab() === "Notifications"}>
            <Pane>
              <Check
                label="Enable notifications"
                checked={s.notifications.enabled}
                onChange={(v) => set((x) => (x.notifications.enabled = v))}
                hint="Shown for the events below in repositories you have open."
              />
              <Check label="Checks failed" checked={s.notifications.checksFailed} onChange={(v) => set((x) => (x.notifications.checksFailed = v))} />
              <Check label="Review requested" checked={s.notifications.reviewRequested} onChange={(v) => set((x) => (x.notifications.reviewRequested = v))} />
              <Check label="Pull request merged" checked={s.notifications.prMerged} onChange={(v) => set((x) => (x.notifications.prMerged = v))} />
              <Check label="Push rejected" checked={s.notifications.pushRejected} onChange={(v) => set((x) => (x.notifications.pushRejected = v))} />
              <Check
                label="Quiet hours"
                checked={!!s.notifications.quietHours}
                onChange={(v) => set((x) => (x.notifications.quietHours = v ? [22, 8] : null))}
                hint="No notifications are shown between these hours."
              />
              <Show when={s.notifications.quietHours}>
                {(quiet) => (
                  <div class="summary-row quiet-hours">
                    <Select
                      value={String(quiet()[0])}
                      options={HOURS}
                      ariaLabel="Quiet hours start"
                      onChange={(v) => set((x) => (x.notifications.quietHours = [Number(v), quiet()[1]]))}
                    />
                    <span class="muted">to</span>
                    <Select
                      value={String(quiet()[1])}
                      options={HOURS}
                      ariaLabel="Quiet hours end"
                      onChange={(v) => set((x) => (x.notifications.quietHours = [quiet()[0], Number(v)]))}
                    />
                  </div>
                )}
              </Show>
            </Pane>
          </Match>
          <Match when={tab() === "Prompts"}>
            <Pane>
              <span class="muted">Show a confirmation dialog before:</span>
              <Check label="Removing repositories" checked={s.prompts.removeRepository} onChange={(v) => set((x) => (x.prompts.removeRepository = v))} />
              <Check label="Discarding changes" checked={s.prompts.discardChanges} onChange={(v) => set((x) => (x.prompts.discardChanges = v))} />
              <Check
                label="Discarding selected lines permanently"
                checked={s.prompts.discardChangesPermanently}
                onChange={(v) => set((x) => (x.prompts.discardChangesPermanently = v))}
                hint="Discarded lines skip the trash and cannot be recovered."
              />
              <Check label="Force pushing" checked={s.prompts.forcePush} onChange={(v) => set((x) => (x.prompts.forcePush = v))} />
              <Check label="Undoing commits" checked={s.prompts.undoCommit} onChange={(v) => set((x) => (x.prompts.undoCommit = v))} />
              <Check label="Hard reset" checked={s.prompts.hardReset} onChange={(v) => set((x) => (x.prompts.hardReset = v))} />
              <Check
                label="Deleting a branch with unpushed commits"
                checked={s.prompts.deleteBranchWithUnpushed}
                onChange={(v) => set((x) => (x.prompts.deleteBranchWithUnpushed = v))}
              />
            </Pane>
          </Match>
          <Match when={tab() === "Advanced"}>
            <Pane>
              <Choice
                label="If I have changes and I switch branches"
                value={s.advanced.branchSwitch}
                options={[
                  { value: "ask", label: "Ask me where I want the changes to go" },
                  { value: "bring-changes", label: "Always bring my changes to my new branch" },
                  { value: "stash-on-current", label: "Always stash and leave my changes on the current branch" },
                ]}
                onChange={(v) => set((x) => (x.advanced.branchSwitch = v))}
              />
              <Check
                label="Periodically fetch and refresh status of all repositories"
                checked={s.advanced.backgroundFetch}
                onChange={(v) => set((x) => (x.advanced.backgroundFetch = v))}
              />
              <Choice
                label="Background fetch interval"
                value={String(s.advanced.backgroundFetchMinutes)}
                options={[
                  { value: "5", label: "Every 5 minutes" },
                  { value: "15", label: "Every 15 minutes" },
                  { value: "30", label: "Every 30 minutes" },
                  { value: "60", label: "Every hour" },
                  { value: "240", label: "Every 4 hours" },
                ]}
                onChange={(v) => set((x) => (x.advanced.backgroundFetchMinutes = Number(v)))}
              />
              <Check
                label="Keep running in the tray when the window is closed"
                checked={s.advanced.closeToTray}
                onChange={(v) => set((x) => (x.advanced.closeToTray = v))}
                hint="Background fetch, checks and notifications keep going. Click the tray icon to bring Blink back, or quit from its menu."
              />
              <Choice
                label="File watcher"
                value={s.advanced.watcher}
                options={[
                  { value: "native", label: "Native file system events" },
                  { value: "fsmonitor", label: "Git fsmonitor" },
                  { value: "polling", label: "Polling (network drives)" },
                ]}
                onChange={(v) => set((x) => (x.advanced.watcher = v))}
                hint="Git fsmonitor needs Windows or macOS and falls back to native events elsewhere."
              />
              <Check
                label="Use system OpenSSH"
                checked={s.advanced.useSystemOpenssh}
                onChange={(v) => set((x) => (x.advanced.useSystemOpenssh = v))}
                hint="Windows only. Uses the OpenSSH client that ships with Windows instead of the one bundled with Git."
              />
              <Choice
                label="Cache size limit"
                value={String(s.advanced.cacheLimitMb)}
                options={[...new Set([128, 256, 512, 1024, 2048, s.advanced.cacheLimitMb])].sort((a, b) => a - b).map((mb) => ({ value: String(mb), label: mb >= 1024 ? `${mb / 1024} GB` : `${mb} MB` }))}
                onChange={(v) => set((x) => (x.advanced.cacheLimitMb = Number(v)))}
                hint="Cached repository views and avatars past this size are removed, oldest first."
              />
              <Choice
                label="Log level"
                value={s.advanced.logLevel}
                options={[
                  { value: "error", label: "Errors only" },
                  { value: "warn", label: "Warnings" },
                  { value: "info", label: "Info" },
                  { value: "debug", label: "Debug" },
                  { value: "trace", label: "Trace (very verbose)" },
                ]}
                onChange={(v) => set((x) => (x.advanced.logLevel = v))}
              />
              <Choice
                label="Update channel"
                value={s.advanced.updateChannel}
                options={[
                  { value: "stable", label: "Stable" },
                  { value: "beta", label: "Beta (prereleases)" },
                ]}
                onChange={(v) => set((x) => (x.advanced.updateChannel = v))}
                hint="Beta gets prerelease builds before they ship to everyone. Prerelease builds of Blink always follow beta."
              />
              <Threads
                label="Foreground threads"
                value={s.advanced.foregroundThreads}
                onChange={(v) => set((x) => (x.advanced.foregroundThreads = v))}
              />
              <Threads
                label="Background threads"
                value={s.advanced.backgroundThreads}
                onChange={(v) => set((x) => (x.advanced.backgroundThreads = v))}
                hint="Thread changes apply after restart."
              />
              <div class="summary-row">
                <button class="btn" onClick={() => api.openLogsFolder().catch(reportError)}>
                  Open logs folder
                </button>
                <button class="btn" onClick={() => setStore("dialog", { type: "import" })}>
                  Import repositories…
                </button>
              </div>
            </Pane>
          </Match>
          <Match when={tab() === "Accessibility"}>
            <Pane>
              <Check label="Underline links" checked={s.accessibility.underlineLinks} onChange={(v) => set((x) => (x.accessibility.underlineLinks = v))} />
              <Check label="Show check marks in the diff" checked={s.accessibility.diffCheckMarks} onChange={(v) => set((x) => (x.accessibility.diffCheckMarks = v))} />
              <Check label="Reduce motion" checked={s.accessibility.reducedMotion} onChange={(v) => set((x) => (x.accessibility.reducedMotion = v))} />
              <Check label="High contrast" checked={s.accessibility.highContrast} onChange={(v) => set((x) => (x.accessibility.highContrast = v))} />
              <Check label="Larger hit targets" checked={s.accessibility.largeHitTargets} onChange={(v) => set((x) => (x.accessibility.largeHitTargets = v))} />
              <Check
                label="Keyboard navigation"
                checked={s.accessibility.keyboardNavigation}
                onChange={(v) => set((x) => (x.accessibility.keyboardNavigation = v))}
                hint="Always show the focus ring, and press F6 or Shift+F6 to move between the toolbar, file list, commit message and diff."
              />
            </Pane>
          </Match>
        </Switch>
      </div>
    </Dialog>
  );
}
