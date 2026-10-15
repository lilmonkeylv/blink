# Changelog

All notable changes to Blink are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow [Semantic Versioning](https://semver.org/).

## [0.1.0-alpha.15] - 2026-10-15

### Fixes

- **sync:** Fetch after a rejected push and when the window comes back, so Pull shows up ([cc96e0e](https://github.com/lilmonkeylv/blink/commit/cc96e0e22b4db2b38e09fa1c8891a830b7d8baa7))

## [0.1.0-alpha.14] - 2026-10-15

### Features

- **ui:** GitHub Desktop style file paths and row dividers ([1ea3f1b](https://github.com/lilmonkeylv/blink/commit/1ea3f1ba4574c8c9f0119fca4eaa861f8da5aec1))

### Fixes

- **changes:** Keep a running commit visible and locked across tab and repo switches ([31b6fd7](https://github.com/lilmonkeylv/blink/commit/31b6fd7cace98d76a76383bc1c50ea346eb948e9))

## [0.1.0-alpha.13] - 2026-10-15

### Fixes

- **diff:** Key diff rows by content so switching files doesn't reuse old row heights ([a76bb35](https://github.com/lilmonkeylv/blink/commit/a76bb3531b11d4a5c29ae430724a3d2ec1f843a6))

## [0.1.0-alpha.12] - 2026-10-14

### Features

- **history:** Mark commits that haven't been pushed yet ([ef3e5fc](https://github.com/lilmonkeylv/blink/commit/ef3e5fc20c606f9154a72f4873a972a7c3ec30d8))

### Fixes

- **sync:** Send every repository refresh to the UI so pushes update the view ([0939736](https://github.com/lilmonkeylv/blink/commit/093973617d28dc67975e1660a52e8ca1b0c87442))

## [0.1.0-alpha.11] - 2026-10-14

### Fixes

- **ui:** Line up the account, menu and window buttons in one row ([5055ee3](https://github.com/lilmonkeylv/blink/commit/5055ee302f812cbca2d53c7c215b0b2f03616528))

## [0.1.0-alpha.10] - 2026-10-14

### Fixes

- **changes:** Clear the commit message as soon as the commit lands ([f9a5322](https://github.com/lilmonkeylv/blink/commit/f9a5322e04a0b50da48896953d00a6ad66069d0a))

## [0.1.0-alpha.9] - 2026-10-14

### Fixes

- **ui:** Give stash rows a rounded hover that fits their buttons ([6cabcee](https://github.com/lilmonkeylv/blink/commit/6cabcee63786740a73c89411aa213863441b2cbd))

## [0.1.0-alpha.8] - 2026-10-14

### Features

- Save crash reports locally and add a CI workflow ([323d315](https://github.com/lilmonkeylv/blink/commit/323d315cec46698fb0443e62a60d980e92aae006))
- **ui:** Show notifications as a bottom-right toast stack ([a1bf8e9](https://github.com/lilmonkeylv/blink/commit/a1bf8e9c64938a65d5372e5e439875f9a18c0f82))
- **ui:** Keyboard navigation and shortcut hints in context menus ([f71af2b](https://github.com/lilmonkeylv/blink/commit/f71af2bfb651293baccbb19fd0eef5123faf6867))
- **ui:** Themed tooltips and controls in place of native ones ([b96f250](https://github.com/lilmonkeylv/blink/commit/b96f250f9385b2d4b2192aff9fa806e3158660a6))
- **history:** Show tags on commits and allow deleting them ([72ba89d](https://github.com/lilmonkeylv/blink/commit/72ba89d1477266b882662fceff2dc73704806223))
- **changes:** Add a stashed changes row below the changes list ([561b70b](https://github.com/lilmonkeylv/blink/commit/561b70b9c201b57de67770a7f9b11d745e844ccc))
- **merge:** Show the common ancestor of each conflict ([0176658](https://github.com/lilmonkeylv/blink/commit/0176658fae7eed7cdc03c4339935cf561218a543))
- **checks:** Expand Actions runs into their jobs ([d90c2cc](https://github.com/lilmonkeylv/blink/commit/d90c2cc3fd906d6c01453e593fca3fe60af22fca))
- **repo:** Copy remote URLs from repository settings ([f30a8d0](https://github.com/lilmonkeylv/blink/commit/f30a8d0b082f8d9cbc6a6ba673e703b8e98f120f))
- **app:** Add an About Blink dialog ([364af6b](https://github.com/lilmonkeylv/blink/commit/364af6b06dd0af484396559025e409668a8e9403))
- Apply git, watcher, logging and cache settings ([23fce85](https://github.com/lilmonkeylv/blink/commit/23fce85ec36c24f83f4d1616fdcf3427a11b31d0))
- **notifications:** Honour quiet hours and every notification switch ([012afe8](https://github.com/lilmonkeylv/blink/commit/012afe8f33cc216f9a6745f82a159781156c2af7))
- **ui:** Wire the remaining appearance, prompt and accessibility settings ([e94c1af](https://github.com/lilmonkeylv/blink/commit/e94c1af55aba00f587b0a242c3cd171870b6bd7a))
- **signing:** Guided commit signing setup in Settings ([a7e80b1](https://github.com/lilmonkeylv/blink/commit/a7e80b14e14d22b5a7e832da823655ac7adfcc94))
- **submodules:** Initialize, update and sync submodules ([61658c3](https://github.com/lilmonkeylv/blink/commit/61658c31fd50d548f89015dfbbaa830617bdb12a))
- **lfs:** Track, pull and lock files with Git LFS ([e2db3cc](https://github.com/lilmonkeylv/blink/commit/e2db3cc58fac4ce7b41ac0b6a11c5b4f281439bf))
- Stream git hook output and offer to skip failing hooks ([9015da3](https://github.com/lilmonkeylv/blink/commit/9015da35b54729de12dca05b12befdc7c9438406))
- Predict merge conflicts before merging or rebasing ([0eeb5fa](https://github.com/lilmonkeylv/blink/commit/0eeb5faf8e1daaea80ff26602fb850c034dee101))
- Bisect from the history list ([479dd66](https://github.com/lilmonkeylv/blink/commit/479dd6643f284cbaed9a3dfcca31eb47adf1f625))
- Keep Blink running in the system tray ([1244e32](https://github.com/lilmonkeylv/blink/commit/1244e32ed8c9c5439f675a0330109a8e21e7c134))
- Support multiple GitHub.com accounts ([faa4742](https://github.com/lilmonkeylv/blink/commit/faa474252bc430c3dc7d473776f580b50b9aec50))

### Fixes

- **windows:** Keep the title bar buttons from taking focus ([25edcf7](https://github.com/lilmonkeylv/blink/commit/25edcf720406af3d2e84552666e3ee1b36d2a431))
- **ui:** Lighten the dark theme and separate the toolbar buttons ([e8eb15d](https://github.com/lilmonkeylv/blink/commit/e8eb15dedf7bedc8eef533a45da346e51895b5c3))
- **changes:** Keep the commit draft per repository ([a57e2ef](https://github.com/lilmonkeylv/blink/commit/a57e2efa2d46d5a6fd9f40957f2d279559fa8691))
- **merge-editor:** Wait for the file before allowing a resolve ([f155105](https://github.com/lilmonkeylv/blink/commit/f1551056b173ec610e31efe38f5a62dc2e019358))
- **clone:** Stop rewriting the URL field while typing ([8800508](https://github.com/lilmonkeylv/blink/commit/8800508b1441351084c45fb233a473f61f65ac97))
- **layout:** Re-clamp the sidebar when the window shrinks ([73fa401](https://github.com/lilmonkeylv/blink/commit/73fa4011a8214d197c22e50f58595e06d45abd9f))
- **ui:** Trap focus in dialogs and pause shortcuts while one is open ([46aee1b](https://github.com/lilmonkeylv/blink/commit/46aee1b17060db6cc9e9c8ca010e96c21d9b5bbc))
- **ui:** Move hardcoded colors onto theme tokens ([1ff1cca](https://github.com/lilmonkeylv/blink/commit/1ff1cca207e0d999e431ae6fa0f94641c8dee835))
- **ui:** Keep panes usable at narrow widths ([a3f101c](https://github.com/lilmonkeylv/blink/commit/a3f101cc4deda6147b8e088f226881b65693045b))
- **ui:** Clearer sign-in dialog states ([1dc45fe](https://github.com/lilmonkeylv/blink/commit/1dc45fe70ced29018853d77edc75fc05e811d420))
- **git:** Correct commit cleanup, pathspecs, push target, index safety and line staging ([be29976](https://github.com/lilmonkeylv/blink/commit/be29976d35a10543c92f1ed427e7c50bff375cc6))
- **security:** Validate deep links, paths and commit ids from the webview ([631a948](https://github.com/lilmonkeylv/blink/commit/631a94849743bfc607708412783e1820cbe7a069))

### Performance

- Keep network git and file watching from stalling the app ([f054fa5](https://github.com/lilmonkeylv/blink/commit/f054fa55329b04f551f68e775f87949e60b39c69))

### Refactoring

- **ui:** Share one delayed loading indicator ([258eb86](https://github.com/lilmonkeylv/blink/commit/258eb8664a97fe860d9cfab509ff280bd2f7f46c))
- **ui:** Build shortcut hints from the keymap ([b093515](https://github.com/lilmonkeylv/blink/commit/b0935156d50d003b39a03d52773ec02c30ae39f4))

## [0.1.0-alpha.7] - 2026-10-13

### Features

- **ui:** Switch the app icons to Lucide ([c6e4589](https://github.com/lilmonkeylv/blink/commit/c6e458914763cb27422b7546e34450104460021c))

### Fixes

- **ui:** Group the app menu into submenus and theme the menu button focus ([52f2da0](https://github.com/lilmonkeylv/blink/commit/52f2da097dc23bd488698685b2f9a694d84e1f84))
- **ui:** Tidy the sidebar footer and commit box, animate the tabs ([830621d](https://github.com/lilmonkeylv/blink/commit/830621d5a2ab0f0fc5fd6c96f6b32727f8beb626))
- **auth:** Bake the GitHub OAuth app into release builds ([53ad922](https://github.com/lilmonkeylv/blink/commit/53ad92272967225a5bcf53f0f78482429a744b4f))

## [0.1.0-alpha.6] - 2026-10-13

### Fixes

- **windows:** Keep the window buttons visible and open at a comfortable size ([c62b895](https://github.com/lilmonkeylv/blink/commit/c62b89589e0dde564607fa395da8bbbf734b7a04))

## [0.1.0-alpha.5] - 2026-10-13

### Fixes

- **ui:** Keep the sync button text on one line ([e3b2636](https://github.com/lilmonkeylv/blink/commit/e3b2636d9d5021068a7867a31fc179ef24d5a59c))
- **windows:** Center and fit the window on launch, put the menu with the window buttons ([8e09428](https://github.com/lilmonkeylv/blink/commit/8e0942864e05c01af3cb43b0157b0542678051c4))

## [0.1.0-alpha.4] - 2026-10-13

### Fixes

- **ui:** Animate the sync button while fetch, pull and push run ([475011f](https://github.com/lilmonkeylv/blink/commit/475011fe8a18b7f2abadfcbf61209e20c8e4c90c))
- **updater:** Prerelease builds follow beta and a missing manifest means no update ([c6b302a](https://github.com/lilmonkeylv/blink/commit/c6b302a10e84901b6bd538c37e9f1684e5a72fc9))
- **ui:** Wait for a repository to open before asking the backend about it ([eb50b2b](https://github.com/lilmonkeylv/blink/commit/eb50b2bad8b1e1936c578d85a593c37288cf6a63))
- **ui:** Keep the sync bar moving until Git reports real transfer progress ([a19f707](https://github.com/lilmonkeylv/blink/commit/a19f707e94607688d1a09d423d1768eb3050f516))

## [0.1.0-alpha.3] - 2026-10-13

### Fixes

- **ui:** In-app prompts instead of native message boxes ([29b07ee](https://github.com/lilmonkeylv/blink/commit/29b07ee23610c10d0188a09a353496fe825408df))
- **git:** Wait out index.lock contention and offer to clear stale locks ([a075fe2](https://github.com/lilmonkeylv/blink/commit/a075fe20d735b48271f2f346f7f271caf9e74048))

## [0.1.0-alpha.2] - 2026-10-13

### Features

- **diff:** Highlight most languages, including TypeScript and TSX ([b2cd636](https://github.com/lilmonkeylv/blink/commit/b2cd636aa75cf8ec4dfd4fed498cfa1b511da4ef))
- **diff:** Side-by-side split view ([2e58224](https://github.com/lilmonkeylv/blink/commit/2e58224742bcd76760ebd6a7d025fc7c7a9b13ea))
- **auth:** Offer to revoke the GitHub grant after signing out ([7e9eddd](https://github.com/lilmonkeylv/blink/commit/7e9eddd2c707e03aa77398157acedef85ae00977))
- **auth:** GitHub Enterprise accounts ([f201e97](https://github.com/lilmonkeylv/blink/commit/f201e97e4e71441773c4c5a7f517c42b94ebb13f))
- **diff:** Show submodule and Git LFS changes ([423be8a](https://github.com/lilmonkeylv/blink/commit/423be8a8d7ecc7babcffd936a8d139f2ebc5f4b8))
- **app:** Open a repository in its own window ([57ff677](https://github.com/lilmonkeylv/blink/commit/57ff677c3e817112656b875112524e3344bb2519))
- **import:** Read the repository list from GitHub Desktop ([2dffa8b](https://github.com/lilmonkeylv/blink/commit/2dffa8b8715820f56810ad84aed63bad6d71439b))
- **app:** Open repositories as tabs ([6f5bb31](https://github.com/lilmonkeylv/blink/commit/6f5bb31492818e9c3f87440281016e1453099787))
- **commit:** Recall recent messages and prefill commit.template ([646c91f](https://github.com/lilmonkeylv/blink/commit/646c91f40165fc424b907c95c4343c40d18eef1c))
- **commit:** Warn about protected branches and unlinked emails ([8ec53c7](https://github.com/lilmonkeylv/blink/commit/8ec53c768ce6793487f32d3033ad35baf5a3eb28))
- **changes:** Filter and multi-select in the changes list ([bcf0847](https://github.com/lilmonkeylv/blink/commit/bcf0847ba1b32661a69959713c316d5ba70b79c0))
- **clone:** Partial clone and sparse checkout ([c1e3f74](https://github.com/lilmonkeylv/blink/commit/c1e3f74b2da13624823bacbcd3667d7e9216b387))
- **stash:** Stash selected lines from the diff ([72b91e7](https://github.com/lilmonkeylv/blink/commit/72b91e7344370b07c3b042674abb2b65b477af27))
- **ui:** Resizable sidebar and per-repository layout memory ([1d48ea6](https://github.com/lilmonkeylv/blink/commit/1d48ea6c2a126a2a00cf2126c8fee70333068444))
- **branches:** Delete on remote, track upstream and prune stale remote branches ([b86f6d5](https://github.com/lilmonkeylv/blink/commit/b86f6d519b5cb49d45102fe181509df7c6a442dc))
- **github:** Explain SAML SSO errors and link to authorize ([0a932d6](https://github.com/lilmonkeylv/blink/commit/0a932d6d3c8be52e64adf0a7b9078cfb153caaad))
- **checks:** Show GitHub Actions job steps and logs ([a6078d1](https://github.com/lilmonkeylv/blink/commit/a6078d17dde0d78bc4bd8f92959a6929049f4612))
- **updater:** Add stable and beta update channels ([1109d7e](https://github.com/lilmonkeylv/blink/commit/1109d7e1c81dd914a6d2415caf4aad92cd1f9ac3))
- **history:** Show a Verified badge on signed commits ([bb17135](https://github.com/lilmonkeylv/blink/commit/bb1713570260136d90a22fb136d15940e5dc60a4))
- **pulls:** Preview a pull request before creating it ([15b05ae](https://github.com/lilmonkeylv/blink/commit/15b05aebf59b6bf87da27c01cbc24440c5514797))
- **sync:** Fetch all repositories with a status overview ([e766fdc](https://github.com/lilmonkeylv/blink/commit/e766fdcc9f13cd6591ee4d45e6d5a11361a7e3df))
- **ui:** Remappable keyboard shortcuts through keymap.toml ([9c2a5ae](https://github.com/lilmonkeylv/blink/commit/9c2a5ae9028b8d005e76f9946b3679dce5233bdc))
- **import:** Bring over editor, shell, theme and OpenSSH from GitHub Desktop ([18b4bb1](https://github.com/lilmonkeylv/blink/commit/18b4bb11a31bfd363f3e20844556af43ae940c1a))
- **windows:** Draw the title bar into the toolbar ([d6cdb69](https://github.com/lilmonkeylv/blink/commit/d6cdb694207db76f9bbf02f593d093dab69eb840))
- **pulls:** Review pull requests in the app ([fc276c9](https://github.com/lilmonkeylv/blink/commit/fc276c9226561f2d7b2a6d84dc96d586ccf43378))

### Fixes

- **ui:** Steady diffs while files change and a graph that fits ([3eb9405](https://github.com/lilmonkeylv/blink/commit/3eb940518c6659562ae679d0881f2595b29dbe4c))
- **settings:** Drop the usage stats checkbox that did nothing ([db563d5](https://github.com/lilmonkeylv/blink/commit/db563d5491d800613b0eb10db0ea161e26330bd9))
- **app:** Keep settings, repositories and accounts in sync across windows ([167dfa3](https://github.com/lilmonkeylv/blink/commit/167dfa349f55b28780fe102e59421be8b48cc2e0))
- **ui:** Repair api.ts after merging the binary IPC and import branches ([81d015b](https://github.com/lilmonkeylv/blink/commit/81d015b9bc232950d24c21bbcdd1f11918f014ac))
- **ui:** Push the tab close button to the right edge ([d5f1a8b](https://github.com/lilmonkeylv/blink/commit/d5f1a8b172d4bf13c400ccf0e2fd0bb453daf5f1))
- **log:** Scrub credentials and tokens from logs and git errors ([498111e](https://github.com/lilmonkeylv/blink/commit/498111ee8b3902e8f6a67f579fd9567a2d8135e6))
- **security:** Load avatars through the backend and drop the remote image host from the CSP ([445711a](https://github.com/lilmonkeylv/blink/commit/445711af7452eb864b1c5e1b5d9ae52862483e1e))

### Performance

- **ipc:** Send diffs and history pages as binary ([fb2989d](https://github.com/lilmonkeylv/blink/commit/fb2989dd897b35f00e4472f321bc385f643e4873))
- **repo:** Paint the last known state of a repository while it opens ([111c8f4](https://github.com/lilmonkeylv/blink/commit/111c8f4d50874f2203b148ffdda4b8b368f4ebcf))
- **github:** Load the pull request list with one GraphQL query ([56cf0f7](https://github.com/lilmonkeylv/blink/commit/56cf0f7d1c78b2fcfafe0a0d0b6ecd8e8cb5dd6e))
- Write the commit-graph after fetches and offer fsmonitor for big repos ([1c143f9](https://github.com/lilmonkeylv/blink/commit/1c143f9a0b682b99f4a19551ce19282eaed09a2e))

### Documentation

- Line the README logo up with the title ([56d5ff9](https://github.com/lilmonkeylv/blink/commit/56d5ff9621e0f838df6faacfe8eb0f136a3a84be))

## [0.1.0-alpha.1] - 2026-10-13

### Features

- P0 workspace, thread pools, OAuth proof and benchmarks ([f01d575](https://github.com/lilmonkeylv/blink/commit/f01d5750ef005a538404ddb6d743c89033606da2))
- P1 local MVP: changes, line staging, commit, history, branches, sync ([d7f2789](https://github.com/lilmonkeylv/blink/commit/d7f27891ad5545a1d2aa3bd0a357fe4dc02b5676))
- P2 GitHub integration: clone, publish, pull requests, checks, forks ([3d86171](https://github.com/lilmonkeylv/blink/commit/3d861711cf97456f0d284f07732d7adbd9f99cd7))
- P3 history rewrites, undo, conflict editor and image diffs ([d595377](https://github.com/lilmonkeylv/blink/commit/d5953770183beb4e33d0b7ea41a0de3cda8765e7))
- P4/P5 graph, blame, search, launch links and roadmap status ([185d539](https://github.com/lilmonkeylv/blink/commit/185d539359c04a8ce2d0e2cad6f9c8c19c4d1180))
- Auto-updater with signed update bundles ([fe78ab3](https://github.com/lilmonkeylv/blink/commit/fe78ab34ca7fc9cbcd128cf5ff41885287c7a390))
- Show what's new after an update ([bd147d0](https://github.com/lilmonkeylv/blink/commit/bd147d0795fd09b66d16535d9667f6975e92a567))

### Fixes

- Surface commit load errors, add devtools shortcut, plan language highlighting ([c601c43](https://github.com/lilmonkeylv/blink/commit/c601c43c66e5acabaaf4275a26a0c4f2f224fef6))
- **watch:** Resolve symlinked roots so macOS events match; bump checkout to v5 ([dbedd65](https://github.com/lilmonkeylv/blink/commit/dbedd65e06008d544ea2ab3d68d5c968049333ca))
- **github:** Build with rand 0.10 ([b724ec4](https://github.com/lilmonkeylv/blink/commit/b724ec432f604c5a0dd28ce8322b109f33d7467f))
- **ui:** Only highlight the current repo and branch in their lists ([a4e6ead](https://github.com/lilmonkeylv/blink/commit/a4e6ead890c1006d3c5d8531cdb1cfb9f0f261f9))

### Documentation

- Add privacy policy, link it and show the logo in the README ([db2519c](https://github.com/lilmonkeylv/blink/commit/db2519c43061d6eb0b181fd989c49626e738226b))

