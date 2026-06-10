# Translations (`docs/i18n/`)

This folder holds every translated copy of GAL's documentation, one subfolder per language. It is a working location, not the policy — the authoritative naming, allowlist, and freshness rules live in [../devguide.md#documentation-conventions](../devguide.md#documentation-conventions). This file is an EN-only signpost; do not translate it.

## Layout

```text
docs/i18n/
└── <lang>/                         e.g. zh-Hant, ja
    ├── README.<lang>.md            ← mirrors the repo-root README.md
    ├── manual.<lang>.md            ← mirrors docs/manual.md
    └── collaborative-tools/
        └── <name>.<lang>.md        ← mirrors docs/collaborative-tools/<name>.md
```

A translation mirrors its canonical source's path and keeps the language in both the folder and the filename. Canonical English docs stay in their normal location and are never moved.

## Current languages

- `zh-Hant` — Traditional Chinese

## Add a translation

1. Pick a canonical source on the translatable allowlist (currently `README.md`, `docs/manual.md`, plus tool docs added on demand — see the devguide).
2. Create `docs/i18n/<lang>/<name>.<lang>.md` at the mirrored path.
3. Start the file with the freshness front-matter:

   ```yaml
   ---
   source: README.md          # repo-relative path to the canonical source
   lang: <lang>               # e.g. ja
   source_commit: PENDING     # replace with the source commit once you commit the in-sync translation
   translated_at: YYYY-MM-DD
   status: current
   ---
   ```

4. Translate the body, keeping headings and anchors aligned with the source.
5. Add a language-switch link at the top of the canonical doc (and back from the translation).
6. After committing, re-stamp `source_commit` with `git log -1 --format=%H -- <source>` so the freshness check reads `current`.

## Add a language

Create `docs/i18n/<lang>/` and add translations as above. The naming, allowlist, and check scale to any number of languages with no schema or tooling change.

## Check freshness

```bash
gal translation-freshness
```

Reports each `(doc, lang)` as `current`, `stale`, or `missing`.
