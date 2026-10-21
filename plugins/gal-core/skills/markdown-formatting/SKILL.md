---
name: markdown-formatting
description: Format Markdown consistently for this project. Use when writing or editing Markdown files, documentation, plans, notes, or README content.
---

# Markdown Formatting Skill

Write clean, consistent Markdown.

## Headings

- Increase heading levels one step at a time
- Use ATX headings: `# Heading`
- Surround headings with blank lines
- Do not repeat the same heading text in the same document

## Lists

- Use `-` for unordered lists
- Indent nested lists with 2 spaces
- Use a single space after the list marker
- Surround lists with blank lines when they are separate blocks

## Code Blocks

- Use fenced code blocks, not indented blocks
- Always specify the language when possible
- Surround code blocks with blank lines

## Lines

- No trailing spaces
- No hard tabs
- No multiple consecutive blank lines
- End files with a single newline

## Line Wrapping

- One paragraph is one physical line. Never hard-wrap prose inside a paragraph, list item, or table cell
- Long lines are correct; `MD013` stays disabled for this reason
- Why this rule exists (do not "fix" it back): agent edits are fragment replacements that cannot re-flow a wrapped paragraph, so hard-wrapped prose degrades into ragged widths after a few edits. And an intra-paragraph line break renders as a space, which corrupts CJK prose with visible gaps between characters
- Exceptions: fenced code blocks, text diagrams, frontmatter, and HTML comment blocks keep their own line structure — there the newlines are content

## Links and Tables

- Do not use bare URLs in prose; prefer `[text](url)`
- Use standard table dividers like `| --- |`

## Punctuation

- In general prose and documentation, do not use CJK fullwidth semicolon punctuation
- Do not pad semicolons with surrounding spaces
- The semicolon rule above does not override Conventional Commit bullets, where semicolons are allowed inside bullet lines

## Recommended `.markdownlint.json`

```json
{
  "MD013": false,
  "MD033": false,
  "MD041": false
}
```

## Output Requirements

When editing Markdown:

1. Preserve structure
2. Normalize formatting to these rules
3. Do not introduce style churn unrelated to the requested change
