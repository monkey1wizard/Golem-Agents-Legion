---
name: install-gal
description: Install or enable the `gal` binary on this machine when the user asks. Use when the user says "install gal", "set up gal", "enable gal", "get gal working", "幫我安裝 gal", "安裝這個 gal", "把 gal 裝起來", or when a `/gal` command fails because the `gal` binary is missing. This skill only carries the install instructions — the AI runs the OS-appropriate package-manager command with the user's consent. It does NOT bundle a binary and does NOT run anything without showing the exact command first.
---

# Install the gal binary

The GAL plugin ships commands, agents, and skills — but every command except this
one needs the `gal` binary on PATH. Installing the plugin alone leaves GAL not yet
usable. Your job: get `gal` installed on this machine, with the user's consent, then
verify it works. Report honestly if you cannot.

There is no "lightweight" GAL: either the binary installs and the post-install
checks pass, or GAL is not yet usable. No fake success.

## Step 1 — Is it already installed?

Run `gal --version`.

- **Succeeds** → the binary is ready. Report the version. If the user also wants
  the machine projections rebuilt, run `gal refresh`; if they want a repo set up,
  tell them to run `gal init` in that repo after refresh. Distinguish "binary
  works" from "projection healthy" — report each separately. Done.
- **Fails / not found** → continue to Step 2.

## Step 2 — Pick the channel for this OS

Detect the OS and choose the first available channel. **Do not assume a channel is
live** — if a probe fails, move to the next one.

- **macOS / Linux → Homebrew first**. Current status: live and externally verified.
  Probe with `brew info monkey1wizard/tap/gal`, then install with
  `brew install monkey1wizard/tap/gal`.
- **Windows → winget first**. Current status: do **not** assume live catalog
  availability; probe with `winget show Monkey1Wizard.GAL` first. Only if that
  succeeds, install with `winget install Monkey1Wizard.GAL`.
- **Any OS with a Rust toolchain → Cargo fallback**. Current status: live and
  verified. Install with
  `cargo install --git https://github.com/monkey1wizard/golem-agents-legion gal-cli --locked`.
- **None of the above** → stop and report honestly (see Step 5).

Channel honesty rules:

- Homebrew: published and verified; still probe first rather than assuming the
  local machine can reach it.
- winget: probe result is authoritative; if `winget show` fails or the package is
  missing, say that Windows package-manager availability is not proven on this
  machine and fall through to Cargo only if Rust is available.
- Cargo: available fallback, but do not call it a complete success until
  `gal refresh` succeeds after install.

## Step 3 — Ask before running

Before running any install command:

1. Show the **exact command** you intend to run.
2. Wait for the user's consent.
3. Only then run it.

Do not pre-approve or auto-run a mutating install command. Let the normal
permission / sandbox prompt be the consent gate. Never run install work at session
start or in the background.

## Step 4 — Verify

After the command finishes, verify by channel:

- **Homebrew / winget**:
  1. Run `gal --version`.
  2. If it succeeds, report the version.
  3. Tell the user the next required steps are `gal refresh`, then `gal init` in
     the target repo.
- **Cargo**:
  1. Run `gal --version`.
  2. Run `gal refresh`.
  3. Only if both succeed, report the version and say the next required step is
     `gal init` in the target repo.

- **Any verification step fails** → the install did not take cleanly. Report what
  happened and the next option; do not claim success.

## Step 5 — Honest reporting

If no channel is available (no reachable Homebrew / no published winget package /
no Rust toolchain), or a managed environment blocks running shell commands, say so
plainly and give the user the concrete manual next step. The valid next steps are:

- install Homebrew and retry the Homebrew probe,
- install Rust and use the Cargo fallback,
- or download a GitHub Release asset manually, put `gal` on `PATH`, then run
  `gal refresh` and `gal init`.

Never report a successful install that did not happen.
