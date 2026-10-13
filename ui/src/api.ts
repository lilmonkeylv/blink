import { invoke, type InvokeArgs } from "@tauri-apps/api/core";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { decodeDiff, decodeGraph, decodeHistory, type WirePayload } from "./lib/wire";

export type ChangeKind =
  | "added"
  | "modified"
  | "deleted"
  | "renamed"
  | "copied"
  | "untracked"
  | "conflicted"
  | "typechange";

export interface FileChange {
  path: string;
  oldPath: string | null;
  staged: ChangeKind | null;
  unstaged: ChangeKind | null;
  kind: ChangeKind;
}

export type HeadState =
  | { type: "branch"; name: string; id: string }
  | { type: "detached"; id: string }
  | { type: "unborn"; name: string };

export interface AheadBehind {
  ahead: number;
  behind: number;
}

export interface RepoView {
  id: string;
  name: string;
  generation: number;
  head: HeadState | null;
  files: FileChange[];
  conflicted: number;
  aheadBehind: AheadBehind | null;
  operation: string | null;
  hasRemote: boolean;
}

export type LineKind = "context" | "added" | "removed";

export interface DiffLine {
  kind: LineKind;
  oldNo: number | null;
  newNo: number | null;
  text: string;
  noNewline: boolean;
  tokens?: [number, number, number][];
}

export interface DiffHunk {
  oldStart: number;
  oldLen: number;
  newStart: number;
  newLen: number;
  lines: DiffLine[];
}

export interface FileDiff {
  binary: boolean;
  tooLarge: boolean;
  additions: number;
  deletions: number;
  hunks: DiffHunk[];
  submodule?: SubmoduleDiff;
  lfs?: LfsDiff;
}

export interface SubmoduleDiff {
  old: string | null;
  new: string | null;
  dirty: boolean;
  commits: [string, string][];
}

export interface LfsPointer {
  oid: string;
  size: number;
}

export interface LfsDiff {
  old: LfsPointer | null;
  new: LfsPointer | null;
}

export interface CommitWarnings {
  protectedBranch: boolean;
  unlinkedEmail: string | null;
  login: string | null;
}

export interface CommitSummary {
  id: string;
  shortId: string;
  summary: string;
  authorName: string;
  authorEmail: string;
  time: number;
  parents: string[];
}

export interface CommitFile {
  path: string;
  oldPath: string | null;
  kind: ChangeKind;
}

export interface CommitDetail {
  commit: CommitSummary;
  body: string;
  committerName: string;
  committerEmail: string;
  committerTime: number;
  files: CommitFile[];
}

export type BranchKind = "local" | "remote";

export interface Branch {
  name: string;
  fullName: string;
  kind: BranchKind;
  target: string;
  upstream: string | null;
  lastCommitTime: number;
}

export interface RepoRecord {
  path: string;
  name: string;
  owner: string | null;
  remoteUrl: string | null;
  lastOpened: number;
}

export interface Account {
  login: string;
  id: number;
  name: string | null;
  email: string | null;
  avatar_url: string;
  html_url: string;
}

export interface DeviceCode {
  device_code: string;
  user_code: string;
  verification_uri: string;
  expires_in: number;
  interval: number;
}

export type Theme = "light" | "dark" | "system";

export interface Settings {
  version: number;
  integrations: {
    editor: string | null;
    customEditorPath: string | null;
    shell: string | null;
    customShellPath: string | null;
    diffTool: "built-in" | "vs-code" | "beyond-compare" | "meld" | "k-diff3";
    terminalOpensAtFile: boolean;
  };
  git: {
    defaultBranch: string;
    signCommits: boolean;
    signingKey: string | null;
    pullStrategy: "merge" | "rebase" | "ff-only";
    pruneOnFetch: boolean;
    executable: "system" | "bundled";
    executablePath: string | null;
  };
  appearance: {
    theme: Theme;
    tabSize: number;
    accentColor: string | null;
    density: "compact" | "comfortable";
    diffFont: string | null;
    diffFontSize: number;
    dateFormat: "relative" | "absolute";
  };
  notifications: {
    enabled: boolean;
    checksFailed: boolean;
    reviewRequested: boolean;
    prMerged: boolean;
    pushRejected: boolean;
    quietHours: [number, number] | null;
  };
  prompts: {
    removeRepository: boolean;
    discardChanges: boolean;
    discardChangesPermanently: boolean;
    forcePush: boolean;
    undoCommit: boolean;
    hardReset: boolean;
    deleteBranchWithUnpushed: boolean;
  };
  advanced: {
    branchSwitch: "ask" | "bring-changes" | "stash-on-current";
    backgroundFetch: boolean;
    backgroundFetchMinutes: number;
    useSystemOpenssh: boolean;
    watcher: "fsmonitor" | "native" | "polling";
    cacheLimitMb: number;
    logLevel: string;
    foregroundThreads: number | null;
    backgroundThreads: number | null;
  };
  accessibility: {
    underlineLinks: boolean;
    diffCheckMarks: boolean;
    reducedMotion: boolean;
    highContrast: boolean;
    largeHitTargets: boolean;
    keyboardNavigation: boolean;
  };
}

export interface Bootstrap {
  repositories: RepoRecord[];
  settings: Settings;
  account: Account | null;
  lastRepository: string | null;
  oauthConfigured: boolean;
  gitFound: boolean;
}

export interface CommandError {
  kind: string;
  message: string;
  detail: string | null;
}

export interface Progress {
  phase: string;
  percent: number | null;
  line: string;
}

export type SyncAction = "fetch" | "pull" | "push";

export interface FileSelection {
  path: string;
  lines?: [number, number][];
}

export interface CommitMessage {
  summary: string;
  description: string;
  coAuthors: string[];
}

export type ResetMode = "soft" | "mixed" | "hard";
export type RebaseAction = "pick" | "reword" | "squash" | "fixup" | "drop";

export interface RebaseStep {
  action: RebaseAction;
  sha: string;
  message?: string | null;
}

export interface UndoEntry {
  id: number;
  label: string;
  action:
    | { type: "moveBranch"; branch: string | null; sha: string }
    | { type: "restoreBranch"; name: string; sha: string };
}

export interface Blame {
  lines: { sha: string; line: number; text: string }[];
  commits: Record<string, { author: string; time: number; summary: string }>;
}

export type SearchKind = "message" | "author" | "content" | "path";

export interface ReflogEntry {
  sha: string;
  selector: string;
  subject: string;
  time: number;
}

export interface GraphEdge {
  from: number;
  to: number;
  color: number;
}

export interface GraphRow {
  lane: number;
  color: number;
  top: GraphEdge[];
  bottom: GraphEdge[];
  width: number;
}

export interface GraphPage {
  commits: CommitSummary[];
  rows: GraphRow[];
}

export interface Worktree {
  path: string;
  head: string | null;
  branch: string | null;
  bare: boolean;
  locked: boolean;
}

export interface ImagePair {
  old: string | null;
  new: string | null;
  oldSize: number;
  newSize: number;
}

export interface GithubRepo {
  id: number;
  name: string;
  full_name: string;
  owner: { login: string };
  private: boolean;
  fork: boolean;
  clone_url: string;
  ssh_url: string;
  html_url: string;
  default_branch: string | null;
  description: string | null;
  pushed_at: string | null;
}

export interface PullRequest {
  number: number;
  title: string;
  html_url: string;
  state: string;
  draft: boolean;
  user: { login: string };
  head: { ref: string; sha: string; repo: { full_name: string; clone_url: string } | null };
  base: { ref: string; sha: string };
  created_at: string;
  updated_at: string;
}

export interface CheckRun {
  id: number;
  name: string;
  status: string;
  conclusion: string | null;
  html_url: string | null;
  check_suite: { id: number } | null;
}

export interface ChecksSummary {
  state: "success" | "failure" | "pending" | "none";
  total: number;
  failed: number;
  pending: number;
  runs: CheckRun[];
  statuses: { context: string; state: string; target_url: string | null }[];
}

export interface ForkInfo {
  isFork: boolean;
  parent: string | null;
  parentUrl: string | null;
  hasUpstream: boolean;
}

export interface RepoSettings {
  remotes: [string, string][];
  gitignore: string;
  userName: string | null;
  userEmail: string | null;
  sparse: string[] | null;
}

export interface Tool {
  id: string;
  name: string;
}

export type Launch = { type: "open"; path: string } | { type: "clone"; url: string };

export function onChecksChanged(handler: (e: { repo: string; summary: ChecksSummary }) => void): Promise<UnlistenFn> {
  return listen<{ repo: string; summary: ChecksSummary }>("checks-changed", (e) => handler(e.payload));
}

export interface EnterpriseAccount {
  host: string;
  account: Account;
}

export interface ReleaseNote {
  version: string;
  date: string | null;
  body: string;
}

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  notes: string | null;
}

export function onUpdateAvailable(handler: (info: UpdateInfo) => void): Promise<UnlistenFn> {
  return listen<UpdateInfo>("update-available", (e) => handler(e.payload));
}

export function onUpdateProgress(handler: (p: { downloaded: number; total: number | null }) => void): Promise<UnlistenFn> {
  return listen<{ downloaded: number; total: number | null }>("update-progress", (e) => handler(e.payload));
}

export function onLaunch(handler: () => void): Promise<UnlistenFn> {
  return listen("launch", () => handler());
}

export function isCommandError(e: unknown): e is CommandError {
  return typeof e === "object" && e !== null && "kind" in e && "message" in e;
}

function wire<T>(decode: (payload: WirePayload) => T) {
  return (payload: unknown): T => {
    const binary = Array.isArray(payload) ? typeof payload[0] === "number" : payload instanceof ArrayBuffer || ArrayBuffer.isView(payload);
    // the browser dev mock answers with plain objects
    return binary ? decode(payload as WirePayload) : (payload as T);
  };
}

const SHARED_CHANGED = "shared-changed";

function invokeShared<T>(cmd: string, args?: InvokeArgs): Promise<T> {
  return invoke<T>(cmd, args).then((result) => {
    void emit(SHARED_CHANGED);
    return result;
  });
}

export function onSharedChanged(handler: () => void): Promise<UnlistenFn> {
  return listen(SHARED_CHANGED, () => handler());
}

export const api = {
  bootstrap: () => invoke<Bootstrap>("bootstrap"),
  sharedState: () => invoke<Pick<Bootstrap, "repositories" | "settings" | "account">>("shared_state"),
  addRepository: (path: string) => invokeShared<RepoRecord>("add_repository", { path }),
  initRepository: (path: string) => invokeShared<RepoRecord>("init_repository", { path }),
  createRepository: (
    parent: string,
    name: string,
    description: string,
    readme: boolean,
    gitignore?: string,
    license?: string,
  ) => invokeShared<RepoRecord>("create_repository", { parent, name, description, readme, gitignore, license }),
  removeRepository: (path: string) => invokeShared<void>("remove_repository", { path }),
  openRepository: (path: string) => invoke<RepoView>("open_repository", { path }),
  cachedRepository: (path: string) => invoke<RepoView | null>("cached_repository", { path }),
  openWindow: (path: string) => invoke<void>("open_window", { path }),
  refreshRepository: (id: string) => invoke<RepoView>("refresh_repository", { id }),
  reveal: (id: string, path?: string) => invoke<void>("reveal_in_file_manager", { id, path }),
  openOnGithub: (id: string) => invoke<void>("open_on_github", { id }),

  fileDiff: (id: string, path: string, ignoreWhitespace = false, full = false, highlight = true) =>
    invoke("file_diff", { id, path, ignoreWhitespace, full, highlight }).then(wire(decodeDiff)),
  commit: (id: string, files: FileSelection[], message: CommitMessage, amend = false) =>
    invoke<string>("commit", { id, files, message, amend }),
  discardFiles: (id: string, paths: string[]) => invoke<void>("discard_files", { id, paths }),
  discardLines: (id: string, path: string, lines: [number, number][]) =>
    invoke<void>("discard_lines", { id, path, lines }),
  ignore: (id: string, pattern: string) => invoke<void>("ignore_path", { id, pattern }),
  undoLastCommit: (id: string) => invoke<void>("undo_last_commit", { id }),
  commitTemplate: (id: string) => invoke<string | null>("commit_template", { id }),
  gitIdentity: (id?: string) => invoke<{ name: string | null; email: string | null }>("git_identity", { id }),
  setGitIdentity: (name: string, email: string) => invoke<void>("set_git_identity", { identity: { name, email } }),

  history: (id: string, offset: number, limit: number, rev?: string) =>
    invoke("history", { id, offset, limit, rev }).then(wire(decodeHistory)),
  commitDetail: (id: string, sha: string) => invoke<CommitDetail>("commit_detail", { id, sha }),
  commitFileDiff: (id: string, sha: string, path: string, oldPath: string | null, ignoreWhitespace = false, highlight = true) =>
    invoke("commit_file_diff", { id, sha, path, oldPath, ignoreWhitespace, highlight }).then(wire(decodeDiff)),

  branches: (id: string) => invoke<Branch[]>("branches", { id }),
  checkout: (id: string, name: string, kind: BranchKind, changes?: "bringAlong" | "stashOnCurrent") =>
    invoke<void>("checkout_branch", { id, name, kind, changes }),
  createBranch: (id: string, name: string, start?: string) => invoke<void>("create_branch", { id, name, start }),
  renameBranch: (id: string, old: string, newName: string) => invoke<void>("rename_branch", { id, old, new: newName }),
  deleteBranch: (id: string, name: string, force: boolean, includeRemote: boolean) =>
    invoke<void>("delete_branch", { id, name, force, includeRemote }),
  deleteRemoteBranch: (id: string, name: string) => invoke<void>("delete_remote_branch", { id, name }),
  setUpstream: (id: string, branch: string, upstream: string | null) => invoke<void>("set_upstream", { id, branch, upstream }),
  pruneRemotes: (id: string) => invoke<string[]>("prune_remotes", { id }),
  merge: (id: string, branch: string, squash = false) =>
    invoke<{ conflicted: boolean; output: string }>("merge_branch", { id, branch, squash }),
  abortMerge: (id: string) => invoke<void>("abort_merge", { id }),
  resolveConflict: (id: string, path: string, ours: boolean) => invoke<void>("resolve_conflict", { id, path, ours }),
  stash: (id: string, message?: string, paths?: string[]) => invoke<void>("stash_changes", { id, message, paths }),
  stashSelection: (id: string, files: FileSelection[], message: string) => invoke<void>("stash_selection", { id, files, message }),
  stashList: (id: string) => invoke<{ reference: string; message: string }[]>("stash_list", { id }),
  stashPop: (id: string, reference: string) => invoke<void>("stash_pop", { id, reference }),
  stashDrop: (id: string, reference: string) => invoke<void>("stash_drop", { id, reference }),

  sync: (id: string, action: SyncAction, forceWithLease = false, tags = false) =>
    invoke<void>("sync", { id, action, forceWithLease, tags }),

  revert: (id: string, sha: string) => invoke<void>("revert_commit", { id, sha }),
  cherryPick: (id: string, shas: string[], target?: string) => invoke<void>("cherry_pick", { id, shas, target }),
  reset: (id: string, sha: string, mode: ResetMode) => invoke<void>("reset_to_commit", { id, sha, mode }),
  checkoutCommit: (id: string, sha: string) => invoke<void>("checkout_commit", { id, sha }),
  tags: (id: string) => invoke<[string, string][]>("tags", { id }),
  createTag: (id: string, name: string, sha: string, message?: string) =>
    invoke<void>("create_tag", { id, name, sha, message }),
  deleteTag: (id: string, name: string) => invoke<void>("delete_tag", { id, name }),
  rebase: (id: string, onto: string) => invoke<void>("rebase_branch", { id, onto }),
  rebaseInteractive: (id: string, plan: { base: string | null; steps: RebaseStep[]; label?: string }) =>
    invoke<void>("rebase_interactive", { id, plan }),
  squash: (id: string, target: string, commits: string[], message: string) =>
    invoke<void>("squash_commits", { id, target, commits, message }),
  continueOperation: (id: string, operation: string) => invoke<void>("continue_operation", { id, operation }),
  abortOperation: (id: string, operation: string) => invoke<void>("abort_operation", { id, operation }),
  skipOperation: (id: string, operation: string) => invoke<void>("skip_operation", { id, operation }),
  undoHistory: (id: string) => invoke<UndoEntry[]>("undo_history", { id }),
  undo: (id: string) => invoke<UndoEntry>("undo", { id }),

  blame: (id: string, path: string, rev?: string) => invoke<Blame>("blame", { id, path, rev }),
  fileHistory: (id: string, path: string, offset: number, limit: number) =>
    invoke<CommitSummary[]>("file_history", { id, path, offset, limit }),
  searchHistory: (id: string, kind: SearchKind, query: string) =>
    invoke<CommitSummary[]>("search_history", { id, kind, query }),
  reflog: (id: string) => invoke<ReflogEntry[]>("reflog", { id }),
  historyGraph: (id: string, offset: number, limit: number) =>
    invoke("history_graph", { id, offset, limit }).then(wire(decodeGraph)),
  rangeFiles: (id: string, oldest: string, newest: string) => invoke<CommitFile[]>("range_files", { id, oldest, newest }),
  rangeFileDiff: (id: string, oldest: string, newest: string, path: string, oldPath: string | null, highlight = true) =>
    invoke("range_file_diff", { id, oldest, newest, path, oldPath, highlight }).then(wire(decodeDiff)),
  compare: (id: string, other: string) =>
    invoke<{ ahead: CommitSummary[]; behind: CommitSummary[] }>("compare_branch", { id, other }),
  worktrees: (id: string) => invoke<Worktree[]>("worktrees", { id }),
  addWorktree: (id: string, path: string, branch: string, create: boolean) =>
    invoke<void>("add_worktree", { id, path, branch, create }),
  removeWorktree: (id: string, path: string, force: boolean) => invoke<void>("remove_worktree", { id, path, force }),
  commitPatch: (id: string, sha: string) => invoke<string>("commit_patch", { id, sha }),
  applyPatch: (id: string, patch: string) => invoke<void>("apply_patch", { id, patch }),
  imagePair: (id: string, path: string, oldPath: string | null, sha?: string) =>
    invoke<ImagePair>("image_pair", { id, path, oldPath, sha }),
  readWorktreeFile: (id: string, path: string) => invoke<string>("read_worktree_file", { id, path }),
  writeResolvedFile: (id: string, path: string, content: string) =>
    invoke<void>("write_resolved_file", { id, path, content }),
  conflictVersions: (id: string, path: string) =>
    invoke<[string | null, string | null, string | null]>("conflict_versions", { id, path }),

  githubRepositories: () => invoke<GithubRepo[]>("github_repositories"),
  githubOrgs: () => invoke<{ login: string; avatar_url: string }[]>("github_orgs"),
  gitignoreTemplates: () => invoke<string[]>("gitignore_templates"),
  clone: (url: string, parent: string, name: string, depth?: number, partial = false) =>
    invokeShared<RepoRecord>("clone_repository", { url, parent, name, depth, partial }),
  publish: (id: string, name: string, description: string, isPrivate: boolean, org?: string) =>
    invokeShared<GithubRepo>("publish_repository", { id, name, description, private: isPrivate, org }),
  pullRequests: (id: string) => invoke<PullRequest[]>("pull_requests", { id }),
  checkoutPullRequest: (id: string, number: number, headRef: string) =>
    invoke<void>("checkout_pull_request", { id, number, headRef }),
  createPullRequest: (id: string) => invoke<void>("create_pull_request", { id }),
  branchChecks: (id: string) => invoke<ChecksSummary | null>("branch_checks", { id }),
  commitWarnings: (id: string) => invoke<CommitWarnings>("commit_warnings", { id }),
  rerunFailedChecks: (id: string, suites: number[]) => invoke<void>("rerun_failed_checks", { id, suites }),
  forkInfo: (id: string) => invoke<ForkInfo>("fork_info", { id }),
  addUpstream: (id: string, url: string) => invoke<void>("add_upstream", { id, url }),

  detectIntegrations: () => invoke<{ editors: Tool[]; shells: Tool[] }>("detect_integrations"),
  openInEditor: (id: string, path?: string) => invoke<void>("open_in_editor", { id, path }),
  openInShell: (id: string, path?: string) => invoke<void>("open_in_shell", { id, path }),
  openLogsFolder: () => invoke<void>("open_logs_folder"),
  openDevtools: () => invoke<void>("open_devtools"),
  checkForUpdate: () => invoke<UpdateInfo | null>("check_for_update"),
  installUpdate: () => invoke<void>("install_update"),
  whatsNew: () => invoke<ReleaseNote[]>("whats_new"),
  releaseNotes: () => invoke<ReleaseNote[]>("release_notes"),
  openExternal: (url: string) => invoke<void>("open_external", { url }),
  takePendingLaunch: () => invoke<Launch[]>("take_pending_launch"),
  repoSettings: (id: string) => invoke<RepoSettings>("repo_settings", { id }),
  saveRepoSettings: (id: string, settings: RepoSettings) => invoke<void>("save_repo_settings", { id, settings }),
  importRepositories: (root: string) => invokeShared<RepoRecord[]>("import_repositories", { root }),
  desktopRepositories: () => invoke<RepoRecord[]>("desktop_repositories"),

  signIn: () => invokeShared<{ account: Account; primaryEmail: string | null }>("sign_in"),
  deviceStart: () => invoke<DeviceCode>("device_start"),
  deviceComplete: (code: DeviceCode) => invokeShared<{ account: Account; primaryEmail: string | null }>("device_complete", { code }),
  signOut: () => invokeShared<void>("sign_out"),
  revokeUrl: () => invoke<string | null>("revoke_url"),
  enterpriseAccounts: () => invoke<EnterpriseAccount[]>("enterprise_accounts"),
  enterpriseSignIn: (host: string, token: string) => invokeShared<EnterpriseAccount>("enterprise_sign_in", { host, token }),
  enterpriseSignOut: (host: string) => invokeShared<void>("enterprise_sign_out", { host }),

  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invokeShared<void>("save_settings", { settings }),
};

export function onRepoChanged(handler: (view: RepoView) => void): Promise<UnlistenFn> {
  return listen<RepoView>("repo-changed", (e) => handler(e.payload));
}

export function onSyncProgress(
  handler: (e: { repo: string; action: SyncAction; progress: Progress }) => void,
): Promise<UnlistenFn> {
  return listen<{ repo: string; action: SyncAction; progress: Progress }>("sync-progress", (e) => handler(e.payload));
}
