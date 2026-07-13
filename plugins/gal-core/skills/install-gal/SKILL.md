---
name: install-gal
description: Install or enable the `gal` binary on this machine when the user asks. Use when the user says "install gal", "set up gal", "enable gal", "get gal working", "幫我安裝 gal", "安裝這個 gal", "把 gal 裝起來", or when a `/gal` command fails because the `gal` binary is missing. This skill only carries the install instructions — the AI runs the OS-appropriate package-manager command with the user's consent. It does NOT bundle a binary and does NOT run anything without showing the exact command first.
---

# Install the gal binary

The GAL plugin ships commands, agents, and skills — but every command except this
one needs the `gal` binary on PATH. Installing the plugin alone leaves GAL not yet
usable. Your job: get `gal` installed on this machine, with the user's consent, then
verify it works. Report honestly if you cannot.

There is no "lightweight" GAL: either `gal --version` works (GAL usable) or it does
not (report the honest next step). No fake success.

## Step 1 — Is it already installed?

Run `gal --version`.

- **Succeeds** → the binary is ready. Report the version. Optionally run `gal doctor`
  and, if the user wants projections rebuilt, `gal refresh`. Distinguish "binary
  works" from "projection healthy" — report each separately. Done.
- **Fails / not found** → continue to Step 2.

## Step 2 — Pick the channel for this OS

Detect the OS and choose the first available channel. **Do not assume a channel is
live** — if a probe fails, move to the next one.

- **macOS / Linux → Homebrew** (when the tap/formula is published):
  `brew install gal` (or the tapped formula). Probe with `brew info` first.
- **Windows → winget** (when the package is published):
  `winget install Monkey1Wizard.GAL`. Probe with `winget show Monkey1Wizard.GAL` first.
- **Any OS with a Rust toolchain → source install** (always available if `cargo` is on PATH):
  `cargo install --git https://github.com/monkey1wizard/golem-agents-legion gal-cli --locked`
- **None of the above** → stop and report honestly (see Step 5).

The package-manager channels (Homebrew / winget) may not be live yet. If a probe
shows the package is not published, say so and fall through to the source install.

## Step 3 — Ask before running

Before running any install command:

1. Show the **exact command** you intend to run.
2. Wait for the user's consent.
3. Only then run it.

Do not pre-approve or auto-run a mutating install command. Let the normal
permission / sandbox prompt be the consent gate. Never run install work at session
start or in the background.

## Step 4 — Verify

After the command finishes, run `gal --version` again.

- **Succeeds** → GAL is usable. Report the version. Optionally offer `gal doctor`.
- **Fails** → the install did not take. Report what happened and the next option;
  do not claim success.

## Step 5 — Honest reporting

If no channel is available (no Homebrew/winget package published and no Rust
toolchain), or a managed environment blocks running shell commands, say so plainly
and give the user the concrete manual next step (install a package manager, install
Rust, or download a release binary when releases are published). Never report a
successful install that did not happen.
