---
name: setup-deploy
description: "One-time deploy configuration. Auto-detects platform (Fly.io, Render, Vercel, Netlify, Heroku, GitHub Actions, or custom), discovers production URL and health endpoints, and writes config to CLAUDE.md. Run once before /land-and-deploy."
---

# /setup-deploy

Configure deployment once so `/land-and-deploy` works every time.

## Role

Deploy configurator. One-time setup, never needs to run again unless infrastructure changes.

## When to Use

- The first time you want to use `/land-and-deploy` on a project
- When deploy infrastructure changes (new platform, new URL, new commands)

## Step 1 — Detect Platform

Scan the project root for these files (in priority order):

| File | Platform |
| --- | --- |
| `fly.toml` | Fly.io |
| `render.yaml` | Render |
| `vercel.json` or `.vercel/` | Vercel |
| `netlify.toml` | Netlify |
| `heroku.yml` or `Procfile` | Heroku |
| `.github/workflows/*.yml` containing `deploy` | GitHub Actions |

If none detected: ask the user "What platform do you deploy to?"

## Step 2 — Discover Config Per Platform

### Fly.io
- App name from `fly.toml` → `[app]`
- Production URL: `https://<app-name>.fly.dev` (ask to confirm or override)
- Deploy command: `fly deploy`
- Status command: `fly status`
- Health endpoint: `/health` or `/` (ask)

### Render
- Service name from `render.yaml`
- Production URL: ask (Render URLs are assigned after first deploy)
- Deploy command: `git push` (Render deploys on push) or Render CLI if installed
- Status command: check Render dashboard URL

### Vercel
- Production URL: `vercel --prod` output, or from `vercel.json`
- Deploy command: `vercel --prod`
- Status command: `vercel ls`

### Netlify
- Production URL: from `netlify.toml` `[context.production]` or Netlify CLI
- Deploy command: `netlify deploy --prod`
- Status command: `netlify status`

### Heroku
- App name from `heroku.yml` or `git remote -v`
- Production URL: `https://<app-name>.herokuapp.com`
- Deploy command: `git push heroku main`
- Status command: `heroku ps`

### GitHub Actions
- Find the deploy workflow file and name
- Deploy command: trigger workflow via `gh workflow run <name>`
- Status command: `gh run list --workflow=<name> --limit 1`
- Production URL: ask the user

### Custom
- Ask for each: deploy command, status command, production URL, health endpoint

## Step 3 — Confirm With User

Show the discovered config and ask: "Is this correct?"

```
Platform:         Fly.io
App:              my-app
Production URL:   https://my-app.fly.dev
Deploy command:   fly deploy
Status command:   fly status
Health endpoint:  /health
```

Allow the user to edit any field before writing.

## Step 4 — Write to CLAUDE.md

Append or update the `## Deploy Configuration` section in `CLAUDE.md`:

```markdown
## Deploy Configuration

**Platform:** Fly.io
**Production URL:** https://my-app.fly.dev
**Deploy command:** `fly deploy`
**Status command:** `fly status`
**Health endpoint:** /health
**Configured:** <date>
```

## Step 5 — Confirm

Tell the user: "Deploy configuration saved to `CLAUDE.md`. You can now use `/land-and-deploy` after a PR is merged."
