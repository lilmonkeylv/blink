<div align="center">

# Blink

**A blazing-fast, multithreaded Git client for GitHub, built with Rust and Tauri.**

Everything you use in GitHub Desktop, without the lag.

[Features](#features) · [Performance](#performance-targets) · [Architecture](#architecture) · [Building](#building-from-source) · [Roadmap](#roadmap) · [Contributing](#contributing)

</div>

---

> **Status: alpha.** All roadmap phases have a first implementation. Expect rough edges; features marked _planned_ are not implemented yet.

## Why Blink?

GitHub Desktop is a great workflow trapped in a heavy runtime. It is an Electron app that spawns a `git` process for nearly every action, ships a full Chromium, and re-renders whole lists on every refresh. On large repositories that means slow startup, laggy diffs and a UI that freezes while Git works.

Blink keeps the workflow and rebuilds the engine:

- **Native Rust core.** Status, history, diffs and blame run in-process through [gitoxide](https://github.com/GitoxideLabs/gitoxide), with no process spawning on the hot path.
- **Multithreaded by design.** CPU work, network I/O and background jobs run on separate thread pools, so a slow fetch never blocks a diff and the UI never waits.
- **Tiny footprint.** Tauri uses the system webview instead of bundling Chromium.
- **Familiar.** Same layout, same settings, same shortcuts as GitHub Desktop. If you know Desktop, you already know Blink.

## Features

### GitHub Desktop parity

- Sign in with GitHub through OAuth (PKCE, system browser) or a device code; GitHub Enterprise _(planned)_
- Clone, create, add and publish repositories
- Changes view with line- and hunk-level staging, discard, ignore
- Unified diffs with syntax highlighting (split view _planned_); image diffs (2-up, swipe, onion skin, difference)
- Syntax highlighting for most languages, including TypeScript/TSX, Vue, Svelte, Lua, Kotlin, Swift, Dockerfile, TOML and GraphQL _(planned; today only the grammars bundled with syntect are covered)_
- Commit with co-authors, amend, undo
- History with revert, cherry-pick, reorder and squash by drag-and-drop
- Branches: create, rename, delete, merge, rebase, squash-merge, compare
- Fetch, pull, push with background fetch and force-with-lease
- Pull requests: list, check out (including forks), create, CI check status with re-run, failure notifications
- Merge conflict resolution with a built-in conflict editor
- External editor and shell integration
- Same settings tabs as Desktop: Accounts, Integrations, Git, Appearance, Notifications, Prompts, Advanced, Accessibility

### Beyond GitHub Desktop

- Commit graph
- File history and blame
- Unlimited named stashes, partial stashes
- Undo for commits, resets, rebases, merges and branch deletes (`Ctrl+Z`), plus a reflog browser
- Built-in conflict editor (ours, theirs, both)
- Interactive rebase screen
- Worktrees; multiple windows and tabbed repositories _(planned)_
- Command palette (`Ctrl/Cmd+K`)
- History search by message, author, path or content
- Compare branches, multi-commit diffs, copy and apply patches
- `blink .` from a terminal and `blink://` links

## Performance targets

Measured on a mid-range laptop. GitHub Desktop figures are approximate baselines to be re-measured.

| Metric                        | GitHub Desktop  | Blink target       |
| ----------------------------- | --------------- | ------------------ |
| Cold start to interactive     | 2-4 s           | < 400 ms           |
| Idle memory                   | 300-600 MB      | < 120 MB           |
| Installer size                | ~150 MB         | < 20 MB            |
| Status refresh, 50k-file repo | 0.5-2 s         | < 100 ms           |
| Open history, 500k commits    | several seconds | < 50 ms first page |
| Switch repository             | 1-3 s           | < 150 ms           |

Benchmarks run in CI on every pull request; a regression over 10% fails the build.

## Architecture

```
┌──────────────────────────────────────────────┐
│ UI · Tauri webview (SolidJS)                 │  renders visible rows only
└───────────────┬──────────────────▲───────────┘
          commands        binary rows + progress events
┌───────────────▼──────────────────┴───────────┐
│ Command router · Tokio multi-thread runtime  │  cancels stale work
└───────────────┬──────────────────────────────┘
┌───────────────▼──────────────┐ ┌─────────────┐
│ Repo actor (one per repo)    │◄┤ File watcher│
│ serialized writes,           │ └─────────────┘
│ parallel lock-free reads     │
└───────┬─────────────┬────────┴─────┬─────────┘
┌───────▼──────┐ ┌────▼───────┐ ┌────▼─────────┐
│ CPU pool     │ │ I/O pool   │ │ Background   │
│ rayon        │ │ Tokio      │ │ low priority │
│ status, diff │ │ GitHub API │ │ fetch-all,   │
│ highlighting │ │ fetch/push │ │ indexing     │
└──────────────┘ └────────────┘ └──────────────┘
   gix · libgit2 · git CLI · SQLite (WAL) · OS keychain
```

- **Main thread** runs only the window loop.
- **Repo actors** order writes one at a time (Git allows a single `index.lock` writer) while reads run in parallel on immutable snapshots.
- **Cancellation** tokens drop stale work the moment you switch file or repository.
- **Git engines:** gitoxide for reads; libgit2 or the Git CLI for writes, hooks and LFS until gitoxide's write paths are proven.

## Tech stack

| Layer   | Choice                                                                                   |
| ------- | ---------------------------------------------------------------------------------------- |
| Shell   | [Tauri 2](https://tauri.app)                                                             |
| Core    | Rust, Tokio, rayon                                                                       |
| Git     | [gix](https://crates.io/crates/gix), [git2](https://crates.io/crates/git2), system `git` |
| UI      | SolidJS, TypeScript, Vite, Bun, TanStack Virtual, CodeMirror 6                           |
| GitHub  | REST + GraphQL, OAuth with PKCE                                                          |
| Storage | SQLite (WAL), OS keychain via `keyring`                                                  |

## Repository layout

```
blink/
├── crates/
│   ├── blink-core/       # repo actors, scheduler, thread pools, snapshots
│   ├── blink-git/        # Git facade over gix / git2 / CLI
│   ├── blink-diff/       # diffing, syntax highlighting, image diffs
│   ├── blink-github/     # OAuth, REST/GraphQL client, polling
│   ├── blink-store/      # SQLite cache, settings, keychain
│   └── blink-watch/      # file watching, fsmonitor
├── src-tauri/            # Tauri app: commands, IPC, updater
├── ui/                   # SolidJS front end
└── benches/              # performance benchmark suite
```

## Building from source

### Prerequisites

- [Rust](https://rustup.rs) (stable)
- [Bun](https://bun.sh) 1.2+ (package manager and script runner; Node.js and npm/pnpm are not needed)
- Git 2.40+
- Tauri system dependencies for your OS, see the [Tauri prerequisites guide](https://tauri.app/start/prerequisites/)
  - Windows: WebView2 (preinstalled on Windows 10/11), MSVC build tools
  - macOS: Xcode Command Line Tools
  - Linux: `webkit2gtk-4.1`, `libssl-dev`, `librsvg2-dev` and friends

### Run in development

```bash
git clone https://github.com/lilmonkeylv/blink.git
cd blink
bun install
bun tauri dev
```

### Production build

```bash
bun tauri build
```

Installers are written to `target/release/bundle/`.

Blink uses Bun everywhere: `src-tauri/tauri.conf.json` runs `bun run dev` and `bun run build` as its `beforeDevCommand` and `beforeBuildCommand`, CI uses `oven-sh/setup-bun`, and only `bun.lock` is committed. Please don't add `package-lock.json` or `pnpm-lock.yaml`.

### Releasing

```bash
bun run version:set 0.2.0
git commit -am "chore: release 0.2.0"
git tag v0.2.0
git push origin master v0.2.0
```

`version:set` also regenerates `CHANGELOG.md` from the commit history with [git-cliff](https://git-cliff.org), so write commits in [conventional](https://www.conventionalcommits.org) style (`feat:`, `fix:`, `perf:`). `ci:`, `chore:`, `build:` and `test:` commits are left out of the notes.

Pushing a `v*` tag runs `.github/workflows/release.yml`. It builds the Windows, macOS and Linux installers and attaches them to a draft release. Check it over and publish it from the Releases page. Tags with a suffix like `-beta.1` or `-rc.1` are marked as prereleases. To rebuild an existing tag, run the workflow by hand from the Actions tab.

### Updater signing

Installed copies of Blink check `releases/latest/download/latest.json` every few hours and offer to restart into the new version. Update bundles are signed with a minisign key whose public half lives in `tauri.conf.json` (`plugins.updater.pubkey`). The release workflow needs the private half as repository secrets:

```bash
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/blink.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --body ""
```

Without the secret, releases still build, just without updater bundles. Keep a backup of the private key somewhere safe. If it's lost, existing installs can't verify new updates. Local `bun tauri build` doesn't need the key.

### GitHub OAuth for local development

1. Create an OAuth App at **GitHub → Settings → Developer settings → OAuth Apps**.
2. Set the callback URL to `http://127.0.0.1/callback` and enable **Device Flow**.
3. Copy `.env.example` to `.env` and fill in:

```bash
BLINK_GITHUB_CLIENT_ID=your_client_id
BLINK_GITHUB_CLIENT_SECRET=your_client_secret
```

Never commit `.env`. Tokens are stored in your OS keychain, never on disk.

### Tests and benchmarks

```bash
cargo test --workspace
cargo bench
bun run test
```

## Roadmap

| Phase                       | Scope                                                                    | Status      |
| --------------------------- | ------------------------------------------------------------------------ | ----------- |
| P0 · Spike and baseline     | Benchmarks, Tauri and thread-pool skeleton, OAuth proof                  | Done        |
| P1 · Local MVP              | Changes, staging, commit, history, branches, merge, stash, sync, sign-in | Done        |
| P2 · GitHub integration     | Clone, publish, PRs, checks, notifications, forks                        | Done        |
| P3 · Rewrites and conflicts | Cherry-pick, reorder, squash, rebase, undo, merge editor                 | Done        |
| P4 · Parity and v1.0        | LFS, submodules, signing, CLI, migration from GitHub Desktop             | In progress |
| P5 · Beyond Desktop         | Commit graph, blame, worktrees, multi-window                             | In progress |
| P6 · Language coverage      | Highlighting grammars for most languages (TS/TSX, Vue, Svelte, Lua)     | Planned     |

## Contributing

Contributions are welcome. Before opening a pull request:

1. Open an issue to discuss larger changes.
2. Run `cargo fmt`, `cargo clippy --workspace -- -D warnings` and `bun run lint`.
3. Make sure tests and benchmarks pass. Performance regressions are treated as bugs.

Correctness comes first: any change touching Git write paths needs a test comparing the result against the Git CLI.

## License

Licensed under [MIT License](LICENSE).

## Acknowledgements

Blink is an independent project and is not affiliated with or endorsed by GitHub. It is inspired by [GitHub Desktop](https://github.com/desktop/desktop) and built on the excellent work of [gitoxide](https://github.com/GitoxideLabs/gitoxide), [libgit2](https://libgit2.org) and [Tauri](https://tauri.app).
