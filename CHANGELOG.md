# Changelog

All notable changes to Blink are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow [Semantic Versioning](https://semver.org/).

## [0.1.0-alpha.4] - 2026-10-13

### Fixes

- **ui:** Animate the sync button while fetch, pull and push run ([475011f](https://github.com/lilmonkeylv/blink/commit/475011fe8a18b7f2abadfcbf61209e20c8e4c90c))
- **updater:** Prerelease builds follow beta and a missing manifest means no update ([c6b302a](https://github.com/lilmonkeylv/blink/commit/c6b302a10e84901b6bd538c37e9f1684e5a72fc9))
- **ui:** Wait for a repository to open before asking the backend about it ([eb50b2b](https://github.com/lilmonkeylv/blink/commit/eb50b2bad8b1e1936c578d85a593c37288cf6a63))

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

