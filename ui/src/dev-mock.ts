import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { Account, Bootstrap, CommitSummary, FileDiff, HookAction, HookEvent, RepoView } from "./api";

const now = Math.floor(Date.now() / 1000);

const repo: RepoView = {
  id: "C:/dev/blink",
  name: "blink",
  generation: 1,
  head: { type: "branch", name: "main", id: "0c0fd03a1b2c3d4e5f60718293a4b5c6d7e8f901" },
  aheadBehind: { ahead: 2, behind: 0 },
  conflicted: 0,
  operation: null,
  hasRemote: true,
  bisecting: false,
  files: [
    { path: "crates/blink-core/src/engine.rs", oldPath: null, staged: null, unstaged: "modified", kind: "modified" },
    { path: "ui/src/App.tsx", oldPath: null, staged: null, unstaged: "modified", kind: "modified" },
    { path: "docs/notes.md", oldPath: null, staged: null, unstaged: "untracked", kind: "untracked" },
    { path: "old.txt", oldPath: null, staged: null, unstaged: "deleted", kind: "deleted" },
    { path: "vendor/libgit2", oldPath: null, staged: null, unstaged: "modified", kind: "modified" },
  ],
};

const diff: FileDiff = {
  binary: false,
  tooLarge: false,
  additions: 3,
  deletions: 1,
  hunks: [
    {
      oldStart: 10,
      oldLen: 5,
      newStart: 10,
      newLen: 7,
      lines: [
        { kind: "context", oldNo: 10, newNo: 10, text: "impl Engine {", noNewline: false, tokens: [[0, 4, 5], [5, 6, 7]] },
        { kind: "context", oldNo: 11, newNo: 11, text: "    pub fn new(config: PoolConfig) -> Self {", noNewline: false },
        { kind: "removed", oldNo: 12, newNo: null, text: "        let (events, _) = broadcast::channel(64);", noNewline: false },
        { kind: "added", oldNo: null, newNo: 12, text: "        let (events, _) = broadcast::channel(256);", noNewline: false },
        { kind: "added", oldNo: null, newNo: 13, text: "        tracing::debug!(?config, \"engine starting\");", noNewline: false, tokens: [[17, 6, 6], [34, 17, 2]] },
        { kind: "added", oldNo: null, newNo: 14, text: "        return Err(Error::Git(\"finish or abort the operation in progress first before trying to undo anything else\".into()));", noNewline: false },
        { kind: "context", oldNo: 13, newNo: 15, text: "        Self {", noNewline: false },
        { kind: "context", oldNo: 14, newNo: 16, text: "            network: Arc::new(Semaphore::new(4)),", noNewline: false },
      ],
    },
  ],
};

diff.hunks.push({
  oldStart: 40,
  oldLen: 40,
  newStart: 42,
  newLen: 0,
  lines: Array.from({ length: 40 }, (_, i) => ({
    kind: "removed" as const,
    oldNo: 40 + i,
    newNo: null,
    text: `${i}:` + "function(){const a=document.querySelector('x');a&&a.addEventListener('click',()=>{});}".repeat(8),
    noNewline: false,
  })),
});

const history: CommitSummary[] = Array.from({ length: 60 }, (_, i) => ({
  id: `${(i + 1).toString(16).padStart(40, "a")}`,
  shortId: `${(i + 1).toString(16).padStart(7, "a")}`,
  summary: i === 0 ? "Add repo actor and snapshot publishing" : `Commit number ${60 - i}`,
  authorName: "Dev User",
  authorEmail: "dev@example.com",
  time: now - i * 5400,
  parents: [],
}));

const opened = new Set<string>();
let bisect: { bad: string; good: string[]; skipped: string[]; current: string } | null = null;

function bisectView() {
  if (!bisect) return null;
  const at = (sha: string) => history.findIndex((c) => c.id === sha);
  const bad = at(bisect.bad);
  const good = Math.min(...bisect.good.map(at), history.length);
  const span = good - bad;
  const found = bisect.good.length > 0 && span <= 1;
  const current = history[at(bisect.current)];
  return {
    ...bisect,
    good: [...bisect.good],
    skipped: [...bisect.skipped],
    currentSummary: current?.summary ?? null,
    revisionsLeft: bisect.good.length && !found ? Math.max(0, Math.floor(span / 2) - 1) : null,
    stepsLeft: bisect.good.length && !found ? Math.max(1, Math.ceil(Math.log2(Math.max(span, 2))) - 1) : null,
    firstBad: found ? bisect.bad : null,
    firstBadSummary: found ? history[bad].summary : null,
  };
}

function moveBisect() {
  if (!bisect || !bisect.good.length) return;
  const at = (sha: string) => history.findIndex((c) => c.id === sha);
  const bad = at(bisect.bad);
  const good = Math.min(...bisect.good.map(at));
  if (good - bad > 1) bisect.current = history[bad + Math.floor((good - bad) / 2)].id;
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

async function mockHooks(id: string, action: HookAction, hooks: string[], lines: string[], failing: string | null) {
  const send = (event: HookEvent) => emit("hook-output", { repo: id, action, event });
  await send({ type: "started", hooks });
  for (const line of lines) {
    await sleep(160);
    await send({ type: "output", line });
  }
  await sleep(200);
  await send({ type: "finished", results: hooks.map((name) => ({ name, code: name === failing ? 1 : 0 })) });
  if (failing) throw { kind: "hookFailed", message: `The ${failing} hook failed`, detail: lines.join("\n") };
}

let tags: [string, string][] = [
  ["v0.2.0", history[0].id],
  ["v0.1.0", history[6].id],
  ["first-preview", history[6].id],
];

const remotes: [string, string][] = [
  ["origin", "https://github.com/lilmonkeylv/blink.git"],
  ["upstream", "git@github.com:desktop/desktop.git"],
];

const jobs = (runId: number) => [
  { id: 1, run_id: runId, name: "build (ubuntu)", status: "completed", conclusion: "success", html_url: null, steps: [] },
  { id: 2, run_id: runId, name: "test (windows)", status: "completed", conclusion: "failure", html_url: null, steps: [] },
  { id: 4, run_id: runId, name: "lint", status: "completed", conclusion: "skipped", html_url: null, steps: [] },
  { id: 3, run_id: runId, name: "bench", status: "in_progress", conclusion: null, html_url: null, steps: [] },
];

const accounts: Account[] = [
  { login: "octocat", id: 1, name: "The Octocat", email: null, avatar_url: "", html_url: "https://github.com/octocat" },
  { login: "octo-work", id: 2, name: "Octo at Work", email: null, avatar_url: "", html_url: "https://github.com/octo-work" },
];
const repoAccounts = new Map<string, string>();

export function installDevMock() {
  const boot: Bootstrap = {
    repositories: [
      { path: repo.id, name: "blink", owner: "lilmonkeylv", remoteUrl: "https://github.com/lilmonkeylv/blink.git", lastOpened: now },
      { path: "C:/dev/desktop", name: "desktop", owner: "desktop", remoteUrl: null, lastOpened: 0 },
    ],
    settings: undefined as never,
    account: accounts[0],
    accounts,
    lastRepository: repo.id,
    oauthConfigured: false,
    gitFound: true,
  };
  mockIPC((cmd, args) => {
    const a = args as Record<string, unknown>;
    switch (cmd) {
      case "bootstrap":
        return { ...boot, settings: defaultSettings() };
      case "shared_state":
        return { repositories: boot.repositories, settings: defaultSettings(), account: accounts[0], accounts };
      case "open_repository":
        return new Promise((resolve) =>
          setTimeout(() => {
            opened.add(String(a.path));
            resolve({ ...repo, id: a.path, name: String(a.path).split("/").pop() });
          }, 400),
        );
      case "cached_repository":
        return { ...repo, id: a.path, name: String(a.path).split("/").pop() };
      case "refresh_repository":
        return { ...repo, id: a.id, name: String(a.id).split("/").pop() };
      case "file_diff":
        if (!opened.has(String(a.id))) throw { kind: "core", message: `repository not open: ${a.id}`, detail: null };
        return diff;
      case "commit_file_diff":
        return diff;
      case "history":
        return history.slice(a.offset as number, (a.offset as number) + (a.limit as number));
      case "commit_detail":
        return {
          commit: history.find((c) => c.id === a.sha) ?? history[0],
          body: "Longer description of the change.",
          committerName: "Dev User",
          committerEmail: "dev@example.com",
          committerTime: now,
          files: [{ path: "crates/blink-core/src/engine.rs", oldPath: null, kind: "modified" }],
        };
      case "branches":
        return [
          { name: "main", fullName: "refs/heads/main", kind: "local", target: "", upstream: "origin/main", lastCommitTime: now },
          { name: "feature/watcher", fullName: "refs/heads/feature/watcher", kind: "local", target: "", upstream: null, lastCommitTime: now - 7200 },
          { name: "origin/release", fullName: "refs/remotes/origin/release", kind: "remote", target: "", upstream: null, lastCommitTime: now - 86400 },
        ];
      case "git_identity":
        return { name: "Dev User", email: "dev@example.com" };
      case "stash_list":
        return [
          { reference: "stash@{0}", message: "On main: !!Blink<feature/watcher>" },
          { reference: "stash@{1}", message: "On main: try a bigger channel" },
        ];
      case "take_pending_launch":
      case "undo_history":
        return [];
      case "tags":
        return tags;
      case "create_tag":
        tags = [...tags, [String(a.name), String(a.sha)]];
        return null;
      case "delete_tag":
        tags = tags.filter(([name]) => name !== a.name);
        return null;
      case "remotes":
        return remotes;
      case "repo_settings":
        return { remotes, gitignore: "target/\nnode_modules/\n", userName: null, userEmail: null, sparse: null };
      case "conflict_versions":
        return [["fn main() {", "    run(8);", "    flush();", "}", ""].join("\n"), null, null];
      case "workflow_run_jobs":
        return jobs(a.runId as number);
      case "plugin:app|version":
        return "0.1.0-alpha.7";
      case "history_graph": {
        const page = history.slice(a.offset as number, (a.offset as number) + (a.limit as number));
        return {
          commits: page,
          rows: page.map((_, i) => ({
            lane: i % 7 === 3 ? 1 : 0,
            color: i % 7 === 3 ? 1 : 0,
            top: i === 0 ? [] : [{ from: 0, to: 0, color: 0 }, ...(i % 7 === 3 || i % 7 === 4 ? [{ from: 1, to: i % 7 === 3 ? 1 : 0, color: 1 }] : [])],
            bottom: i === page.length - 1 ? [] : [{ from: 0, to: 0, color: 0 }, ...(i % 7 === 2 || i % 7 === 3 ? [{ from: i % 7 === 2 ? 0 : 1, to: 1, color: 1 }] : [])],
            width: 2,
          })),
        };
      }
      case "detect_integrations":
        return { editors: [{ id: "code", name: "Visual Studio Code" }], shells: [{ id: "wt", name: "Windows Terminal" }, { id: "pwsh", name: "PowerShell 7" }] };
      case "branch_checks":
        return { state: "failure", total: 4, failed: 1, pending: 1, statuses: [], runs: [
          { id: 1, name: "build (ubuntu)", status: "completed", conclusion: "success", html_url: "https://github.com/lilmonkeylv/blink/actions/runs/7/job/1", check_suite: { id: 9 }, app: { slug: "github-actions" } },
          { id: 2, name: "test (windows)", status: "completed", conclusion: "failure", html_url: "https://github.com/lilmonkeylv/blink/actions/runs/7/job/2", check_suite: { id: 9 }, app: { slug: "github-actions" } },
          { id: 3, name: "bench", status: "in_progress", conclusion: null, html_url: null, check_suite: { id: 9 }, app: { slug: "github-actions" } },
          { id: 5, name: "codecov/patch", status: "completed", conclusion: "neutral", html_url: null, check_suite: { id: 11 }, app: { slug: "codecov" } },
        ] };
      case "workflow_job":
        return { id: a.jobId, run_id: 7, name: "test (windows)", status: "completed", conclusion: "failure", html_url: null, steps: [
          { number: 1, name: "Set up job", status: "completed", conclusion: "success" },
          { number: 2, name: "cargo test", status: "completed", conclusion: "failure" },
          { number: 3, name: "Upload artifacts", status: "completed", conclusion: "skipped" },
        ] };
      case "job_log":
        return [
          ...Array.from({ length: 300 }, (_, i) => `2026-10-13T08:00:00.0000000Z test case_${i} ... ok`),
          "2026-10-13T08:00:01.0000000Z thread 'watcher' panicked at crates/blink-watch/src/lib.rs:42:9",
          "2026-10-13T08:00:01.1000000Z ##[error]Process completed with exit code 101.",
        ].join("\n");
      case "pull_requests":
        return [{ number: 42, title: "Add commit graph lanes", html_url: "", state: "open", draft: false, user: { login: "octocat" },
          head: { ref: "feature/graph", sha: "", repo: null }, base: { ref: "main", sha: "" }, created_at: new Date().toISOString(), updated_at: new Date().toISOString(), checks: "success" }];
      case "compare_branch":
        return { ahead: history.slice(0, 2), behind: history.slice(5, 8) };
      case "search_history":
        return history.filter((c) => c.summary.toLowerCase().includes(String(a.query).toLowerCase()));
      case "blame":
        return { lines: diff.hunks[0].lines.filter((l) => l.newNo).map((l, i) => ({ sha: history[i % 3].id, line: i + 1, text: l.text })),
          commits: Object.fromEntries(history.slice(0, 3).map((c) => [c.id, { author: c.authorName, time: c.time, summary: c.summary }])) };
      case "desktop_preferences":
        return { editor: "code", editorName: "Visual Studio Code", shell: "wt", shellName: "Windows Terminal", theme: "dark", useSystemOpenssh: true };
      case "desktop_repositories":
        return [
          { path: "C:/dev/website", name: "website", owner: "octocat", remoteUrl: null, lastOpened: 0 },
          { path: "C:/dev/notes", name: "notes", owner: null, remoteUrl: null, lastOpened: 0 },
        ];
      case "commit_warnings":
        return { protectedBranch: false, unlinkedEmail: null, login: "octocat" };
      case "commit_signature":
        return a.sha === history[0].id ? { state: "verified", reason: null } : null;
      case "pull_request_preview":
        return { base: a.base ?? "main", bases: ["main", "release"], mergeBase: history[3].id, commits: history.slice(0, 3), files: [{ path: "crates/blink-core/src/engine.rs", oldPath: null, kind: "modified" }] };
      case "between_file_diff":
        return diff;
      case "fetch_all":
        return [
          { path: repo.id, name: "blink", fetched: true, error: null, ahead: 2, behind: 0, changes: 4 },
          { path: "C:/dev/desktop", name: "desktop", fetched: false, error: "fatal: could not read Username for 'https://github.com'", ahead: 0, behind: 0, changes: 0 },
        ];
      case "unpushed_commits":
        return 1;
      case "check_git_executable":
        return "2.47.1.windows.1";
      case "keymap":
        return {};
      case "pull_request_review":
        return {
          detail: { number: 42, title: "Add commit graph lanes", body: "Draws **lanes** next to history.", html_url: "", user: { login: "octocat" } },
          files: [{ path: "crates/blink-core/src/engine.rs", oldPath: null, status: "modified", diff }],
          comments: [{ id: 1, body: "Looks good so far", user: { login: "hubot" }, created_at: new Date().toISOString() }],
          reviewComments: [{ id: 2, path: "crates/blink-core/src/engine.rs", line: 12, original_line: 12, side: "RIGHT", body: "Why 256?", user: { login: "hubot" }, created_at: new Date().toISOString(), in_reply_to_id: null }],
          reviews: [],
        };
      case "bisect_state":
        return bisectView();
      case "bisect_start":
        bisect = { bad: String(a.bad), good: a.good ? [String(a.good)] : [], skipped: [], current: repo.head?.type === "branch" ? history[0].id : "" };
        repo.bisecting = true;
        moveBisect();
        return bisectView();
      case "bisect_mark": {
        if (!bisect) return null;
        const sha = String(a.sha ?? bisect.current);
        if (a.mark === "good") bisect.good.push(sha);
        else if (a.mark === "bad") bisect.bad = sha;
        else bisect.skipped.push(sha);
        moveBisect();
        if (a.mark === "skip" && bisect.current === sha) {
          const next = history[history.findIndex((c) => c.id === sha) + 1];
          if (next) bisect.current = next.id;
        }
        return bisectView();
      }
      case "bisect_reset":
        bisect = null;
        repo.bisecting = false;
        return null;
      case "predict_merge":
        return sleep(250).then(() => {
          if (a.branch === "feature/watcher") return { type: "conflicts", files: ["crates/blink-watch/src/lib.rs", "ui/src/App.tsx"] };
          if (a.branch === "origin/release") return { type: "upToDate" };
          return { type: "clean" };
        });
      case "commit": {
        if (a.noVerify) return history[0].id;
        const failing = String((a.message as { summary?: string })?.summary ?? "").includes("fail-hook") ? "pre-commit" : null;
        const lines = ["> lint-staged", "Running tasks for staged files...", "eslint --fix ui/src/App.tsx"];
        if (failing) lines.push("ui/src/App.tsx:12:7  error  'unused' is assigned a value but never used", "1 problem (1 error, 0 warnings)");
        return mockHooks(String(a.id), "commit", ["pre-commit", "commit-msg"], lines, failing).then(() => history[0].id);
      }
      case "sync":
        if (a.action === "push" && !a.noVerify) {
          return mockHooks(String(a.id), "push", ["pre-push"], ["running cargo test", "test result: ok. 214 passed"], null).then(() => sleep(1000)).then(() => null);
        }
        return new Promise((resolve) => setTimeout(() => resolve(null), 1500));
      case "whats_new":
        return [];
      case "release_notes":
        return [
          {
            version: "0.2.0",
            date: "2026-10-20",
            body: [
              "### Features",
              "",
              "- **history:** Commit graph next to the history list ([abc1234](https://github.com/lilmonkeylv/blink/commit/abc1234))",
              "- Auto-updater with signed `latest.json`",
              "",
              "### Fixes",
              "",
              "- Diff rows no longer overlap on very long lines",
            ].join("\n"),
          },
          { version: "0.1.0", date: "2026-10-13", body: "### Features\n\n- First release" },
        ];
      case "read_worktree_file":
        return new Promise((resolve) =>
          setTimeout(
            () => resolve(["fn main() {", "<<<<<<< HEAD", "    run(64);", "=======", "    run(256);", ">>>>>>> feature", "}", ""].join("\n")),
            800,
          ),
        );
      case "lfs_status":
        return { version: "git-lfs/3.7.1 (GitHub; windows amd64; go 1.25.1)", usesLfs: true };
      case "lfs_locks":
        return [{ id: "1", path: "ui/src/App.tsx", owner: "hubot", lockedAt: new Date().toISOString() }];
      case "lfs_pull":
        return new Promise((resolve) => setTimeout(() => resolve(null), 1500));
      case "submodules":
        return [
          { name: "vendor/libgit2", path: "vendor/libgit2", url: "https://github.com/libgit2/libgit2.git", initialized: true },
          { name: "docs/theme", path: "docs/theme", url: "https://github.com/acme/theme.git", initialized: false },
        ];
      case "submodule_update":
        return new Promise((resolve) => setTimeout(() => resolve(null), 1500));
      case "open_submodule":
        return { path: `${a.id}/${a.path}`, name: String(a.path).split("/").pop(), owner: null, remoteUrl: null, lastOpened: now };
      case "signing_status":
        return { format: "ssh", key: "C:/Users/dev/.ssh/id_ed25519.pub", scope: "global", enabled: true, gpgFound: false, sshKeygenFound: true };
      case "signing_keys":
        if (a.format === "gpg") throw { kind: "gpgNotFound", message: "GnuPG is not installed", detail: null };
        return [
          { format: "ssh", id: "C:/Users/dev/.ssh/id_ed25519.pub", label: "dev@example.com", detail: "id_ed25519.pub - ssh-ed25519" },
          { format: "ssh", id: "C:/Users/dev/.ssh/work.pub", label: "work laptop", detail: "work.pub - ssh-rsa" },
        ];
      case "generate_ssh_signing_key":
        return { format: "ssh", id: `C:/Users/dev/.ssh/${a.name}.pub`, label: a.comment, detail: `${a.name}.pub - ssh-ed25519` };
      case "github_signing_key":
        return { login: "octocat", added: String(a.key).endsWith("id_ed25519.pub"), verifiedEmails: [], unverifiedEmails: [] };
      case "add_signing_key_to_github":
        throw { kind: "missingScope", message: "Blink's GitHub sign-in isn't allowed to add signing keys. Copy the public key and add it on GitHub instead.", detail: "https://github.com/settings/keys" };
      case "signing_public_key":
        return "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBk0zU1Xv5xmOcBqNnkTAo5f1q2T8ZKcjzkQzRrH7Vbb";
      case "github_accounts":
        return accounts;
      case "repo_account":
        return { account: accounts.find((x) => x.login === repoAccounts.get(String(a.id))) ?? accounts[0], chosen: repoAccounts.get(String(a.id)) ?? null };
      case "set_repo_account":
        if (a.login) repoAccounts.set(String(a.id), String(a.login));
        else repoAccounts.delete(String(a.id));
        return null;
      case "github_repositories":
        return [];
      default:
        return null;
    }
  }, { shouldMockEvents: true });
}

function defaultSettings() {
  return {
    version: 1,
    integrations: { editor: null, customEditorPath: null, shell: null, customShellPath: null, diffTool: "built-in", terminalOpensAtFile: false },
    git: { defaultBranch: "main", signCommits: false, signingKey: null, pullStrategy: "merge", pruneOnFetch: false, executable: "system", executablePath: null },
    appearance: { theme: "system", tabSize: 8, accentColor: null, density: "comfortable", diffFont: null, diffFontSize: 12, dateFormat: "relative" },
    notifications: { enabled: true, checksFailed: true, reviewRequested: true, prMerged: false, pushRejected: true, quietHours: null },
    prompts: { removeRepository: true, discardChanges: true, discardChangesPermanently: true, forcePush: true, undoCommit: true, hardReset: true, deleteBranchWithUnpushed: true },
    advanced: { branchSwitch: "ask", backgroundFetch: true, backgroundFetchMinutes: 60, useSystemOpenssh: false, watcher: "native", cacheLimitMb: 512, logLevel: "info", foregroundThreads: null, backgroundThreads: null, updateChannel: "stable", closeToTray: false },
    accessibility: { underlineLinks: false, diffCheckMarks: false, reducedMotion: false, highContrast: false, largeHitTargets: false, keyboardNavigation: false },
  };
}
