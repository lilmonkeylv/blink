# Privacy Policy

_Last updated: October 13, 2026_

Blink is a desktop Git client. It runs on your computer, works on repositories that are already on your computer, and has no servers of its own. This page explains what Blink stores, what it sends over the network, and who it sends it to.

The short version: **Blink does not collect analytics, telemetry or crash reports, and nothing about you or your code is sent to us.** The only services Blink talks to are GitHub and the Git remotes you configure.

## What Blink stores on your computer

Everything below stays on your machine. None of it is uploaded anywhere by Blink.

| What | Why | Where |
| --- | --- | --- |
| Settings | Your preferences (theme, editor, Git behaviour and so on) | `settings.toml` in the config folder |
| Repository list | The repositories you've added, their paths and remote URLs, so the list and your last open repository come back next time | `cache.sqlite3` in the data folder |
| GitHub account profile | Your GitHub username, user ID, display name, public email and avatar URL, shown in the app once you sign in | `cache.sqlite3` in the data folder |
| GitHub access token | Lets Blink call the GitHub API and push to GitHub on your behalf | Your operating system's credential store (Windows Credential Manager, macOS Keychain, or the Secret Service on Linux) under `dev.blink.app`. Never written to a file |
| Logs | Errors and warnings to help you debug problems. They can contain file paths, branch names and Git output | `logs/blink.log` in the data folder |

Folder locations:

- **Windows:** config in `%APPDATA%\Blink\config`, data in `%LOCALAPPDATA%\Blink\data`
- **macOS:** `~/Library/Application Support/Blink`
- **Linux:** config in `~/.config/blink`, data in `~/.local/share/blink`

Logs are never sent anywhere automatically. If you attach them to a bug report, check them for anything you'd rather not share first.

## What Blink sends over the network

### GitHub, only if you sign in

Signing in happens in your own browser on github.com. Blink never sees your password. GitHub hands Blink an access token, which is used for:

- reading your profile and email addresses, so Blink can fill in your Git name and email if they're not set
- listing your repositories and organizations when you clone or publish
- creating a repository when you publish one
- listing pull requests and checking out their branches
- reading CI check and status results for the branch you have open. This is checked about every 90 seconds while the repository is open
- re-running failed checks when you ask
- fetching `.gitignore` and license templates when you create a repository
- loading your avatar image from `avatars.githubusercontent.com`
- authenticating Git operations against `https://github.com` remotes. The token is only ever handed to Git for `https://github.com` URLs, never for other hosts

All of this goes directly from your computer to GitHub. GitHub's own [privacy statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement) covers what GitHub does with it. Signing out deletes the token from your credential store and the profile from the cache. You can also revoke Blink's access at any time from [GitHub settings](https://github.com/settings/applications).

### Your Git remotes

When you fetch, pull, push or clone, Git talks to whatever remote is configured for that repository (GitHub, GitLab, your own server and so on). Blink uses your normal Git setup for this, including your SSH keys and credential helpers.

**Background fetch** is turned on by default and fetches the repositories you have open about once an hour, so ahead/behind counts stay current. Turn it off in Settings > Advanced.

### Update checks

A few seconds after launch, and then every six hours, Blink downloads a small `latest.json` file from this project's GitHub releases to see whether a newer version is available. If you choose to update, the installer is downloaded from the same place. GitHub can see your IP address and that the request came from Blink, as with any download from GitHub. No identifier for you or your machine is included.

### Links you open

Links you click in Blink (a commit, a pull request, "View on GitHub", release notes) open in your default browser.

## What Blink does not do

- No analytics, usage tracking or telemetry
- No crash reporting service
- No advertising or third-party SDKs
- No access to repositories you haven't added to Blink
- No selling or sharing of data, because we never receive any

Settings > Advanced has a "submit usage stats" checkbox. It is off by default, and right now nothing is wired up to it, so it sends nothing whether it's on or off. If usage statistics are ever added, they will stay opt-in and this policy will be updated before they ship.

## Removing your data

- **Sign out** in Settings > Accounts to delete the GitHub token and profile
- **Remove a repository** from the repository list to forget it. The folder on disk is left alone
- **Uninstall** Blink and delete the folders listed above to remove everything it stored

## Children

Blink is a developer tool and isn't aimed at children. It collects nothing from anyone, children included.

## Changes

If this policy changes, the update will be committed to this file with a new date at the top. The full history is in the repository's commit log.

## Contact

Questions or concerns: [open an issue](https://github.com/lilmonkeylv/blink/issues).
