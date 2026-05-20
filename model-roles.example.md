# Model Roles — Example Setup

> **This is a personal configuration example.**
> Copy this file to `model-roles.local.md` and customize it.
> `model-roles.local.md` is git-ignored and won't be overwritten by upstream updates.

## Core Principle

Use different models for plan review, testing, and final review whenever practical.
The goal is to avoid self-approval and keep verification independent from authorship.
Reviewer work should be higher-level than tester work when possible.

## Machines

| Machine | OS | LLM Resources |
| --- | --- | --- |
| Windows PC | Windows 11 | Copilot, Antigravity CLI, Ollama (GPU) |
| Mac Mini | macOS | Copilot, Antigravity CLI, Ollama (Apple Silicon) |

## Current Mapping

| Role | Machine A | Machine B | Notes |
| --- | --- | --- | --- |
| ARCHITECT | Claude Opus 4.6 | Different model from the planning author when practical | Adversarial plan review |
| DESIGNER | GPT-5.4 | Different model from CODER | Visual design, UX flow, accessibility, design-system review |
| RESEARCHER | GPT-5.4 | Frontier model for research flow | Owns RESEARCH, SYNTHESIZE, and CROSS-REVIEW |
| RESEARCH-VERIFIER | Antigravity CLI (Gemini 2.5 Pro) | Different model from RESEARCHER | Reverse-checks references during VERIFY |
| CODER | Copilot Agent Mode | VS Code Copilot | Main implementation agent |
| TESTER | Antigravity CLI (Gemini 2.5 Pro) | Antigravity CLI | Spec-driven tests and browser QA; different model from CODER |
| REVIEWER | Copilot (GPT 4.1) or Antigravity CLI | Antigravity CLI | Higher-level review than tester |
| SECURITY | Copilot Agent Mode or Antigravity CLI | Different model from CODER when practical | OWASP and STRIDE audit before release-sensitive work |
| RELEASER | Copilot Agent Mode | Same machine as CODER is acceptable | Release prep, deploy orchestration, and doc sync |
| LOCAL | Ollama: Breeze2-8B, TAIDE-LX-8B | Ollama: larger models | Privacy-sensitive and local-language tasks |

## Ollama Models

| Model | Best For |
| --- | --- |
| `willqiu/Llama-Breeze2-8B-Instruct` | Traditional Chinese tasks |
| `TAIDE-LX-8B` | Traditional Chinese, Taiwan-specific |
| `gemma3:latest` | General, multilingual |
| `mistral:latest` | Fast general purpose |

## Migration Examples

When you switch to a new tool, update the mapping table:

| Scenario | What Changes |
| --- | --- |
| Adopt Claude Code | CODER → Claude Code; run `Setup-Machine` to install Claude runtime targets, then run `Sync-DevContext` to generate `CLAUDE.md` |
| Adopt OmO/OpenCode | All roles → OmO discipline agents; AGENTS.md already generated |
| Adopt another supported runtime | Update the table; run `Setup-Machine` to install that runtime's targets and regenerate any repo-local adapters or rule shims it needs |
| Better local model | LOCAL → Ollama (new model); no other changes |
| Drop Copilot entirely | Remove copilot-skills/ symlinks; conventions/ still works everywhere |
| New machine | Clone repo; run setup script; update Machines table |
