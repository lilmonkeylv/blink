import { mockIPC } from "@tauri-apps/api/mocks";
import type { Bootstrap, CommitSummary, FileDiff, RepoView } from "./api";

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
  files: [
    { path: "crates/blink-core/src/engine.rs", oldPath: null, staged: null, unstaged: "modified", kind: "modified" },
    { path: "ui/src/App.tsx", oldPath: null, staged: null, unstaged: "modified", kind: "modified" },
    { path: "docs/notes.md", oldPath: null, staged: null, unstaged: "untracked", kind: "untracked" },
    { path: "old.txt", oldPath: null, staged: null, unstaged: "deleted", kind: "deleted" },
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

export function installDevMock() {
  const boot: Bootstrap = {
    repositories: [
      { path: repo.id, name: "blink", owner: "lilmonkeylv", remoteUrl: "https://github.com/lilmonkeylv/blink.git", lastOpened: now },
      { path: "C:/dev/desktop", name: "desktop", owner: "desktop", remoteUrl: null, lastOpened: 0 },
    ],
    settings: undefined as never,
    account: null,
    lastRepository: repo.id,
    oauthConfigured: false,
    gitFound: true,
  };
  mockIPC((cmd, args) => {
    const a = args as Record<string, unknown>;
    switch (cmd) {
      case "bootstrap":
        return { ...boot, settings: defaultSettings() };
      case "open_repository":
        return { ...repo, id: a.path, name: String(a.path).split("/").pop() };
      case "refresh_repository":
        return { ...repo, id: a.id, name: String(a.id).split("/").pop() };
      case "file_diff":
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
      case "take_pending_launch":
      case "undo_history":
      case "tags":
        return [];
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
        return { state: "failure", total: 3, failed: 1, pending: 1, statuses: [], runs: [
          { id: 1, name: "build (ubuntu)", status: "completed", conclusion: "success", html_url: null, check_suite: { id: 9 }, app: { slug: "github-actions" } },
          { id: 2, name: "test (windows)", status: "completed", conclusion: "failure", html_url: null, check_suite: { id: 9 }, app: { slug: "github-actions" } },
          { id: 3, name: "bench", status: "in_progress", conclusion: null, html_url: null, check_suite: { id: 9 }, app: { slug: "github-actions" } },
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
      case "desktop_repositories":
        return [
          { path: "C:/dev/website", name: "website", owner: "octocat", remoteUrl: null, lastOpened: 0 },
          { path: "C:/dev/notes", name: "notes", owner: null, remoteUrl: null, lastOpened: 0 },
        ];
      case "commit_warnings":
        return { protectedBranch: false, unlinkedEmail: null, login: "octocat" };
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
      case "account":
        return null;
      case "plugin:event|listen":
        return 1;
      default:
        return null;
    }
  });
}

function defaultSettings() {
  return {
    version: 1,
    integrations: { editor: null, customEditorPath: null, shell: null, customShellPath: null, diffTool: "built-in", terminalOpensAtFile: false },
    git: { defaultBranch: "main", signCommits: false, signingKey: null, pullStrategy: "merge", pruneOnFetch: false, executable: "system", executablePath: null },
    appearance: { theme: "system", tabSize: 8, accentColor: null, density: "comfortable", diffFont: null, diffFontSize: 12, dateFormat: "relative" },
    notifications: { enabled: true, checksFailed: true, reviewRequested: true, prMerged: false, pushRejected: true, quietHours: null },
    prompts: { removeRepository: true, discardChanges: true, discardChangesPermanently: true, forcePush: true, undoCommit: true, hardReset: true, deleteBranchWithUnpushed: true },
    advanced: { branchSwitch: "ask", backgroundFetch: true, backgroundFetchMinutes: 60, useSystemOpenssh: false, watcher: "native", cacheLimitMb: 512, logLevel: "info", foregroundThreads: null, backgroundThreads: null, updateChannel: "stable" },
    accessibility: { underlineLinks: false, diffCheckMarks: false, reducedMotion: false, highContrast: false, largeHitTargets: false, keyboardNavigation: false },
  };
}
