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
| Windows PC | Windows 11 | Copilot, Gemini CLI, Ollama (GPU) |
| Mac Mini | macOS | Copilot, Gemini CLI, Ollama (Apple Silicon) |

## Current Mapping

| Role | Machine A | Machine B | Notes |
| --- | --- | --- | --- |
| PLANNER | Copilot (Claude Sonnet 4.6) | Async plan agent | Drafts executable plans |
| ARCHITECT | Claude Opus 4.6 | Different model from PLANNER | Adversarial plan review |
| DESIGNER | GPT-5.4 | Different model from CODER | Visual design, UX flow, accessibility, design-system review |
| RESEARCHER | GPT-5.4 | Frontier model for research flow | Owns RESEARCH and SYNTHESIZE |
| CODER | Copilot Agent Mode | VS Code Copilot | Main implementation agent |
| TESTER | Gemini CLI (Gemini 2.5 Pro) | Gemini CLI | Basic unit/integration tests |
| REVIEWER | Copilot (GPT 4.1) or Gemini CLI | Gemini CLI | Higher-level review than tester |
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
| Adopt Claude Code | CODER → Claude Code; run `Sync-DevContext` to generate CLAUDE.md |
| Adopt OmO/OpenCode | All roles → OmO discipline agents; AGENTS.md already generated |
| Adopt Antigravity | Update table; add adapter in sync script |
| Better local model | LOCAL → Ollama (new model); no other changes |
| Drop Copilot entirely | Remove copilot-skills/ symlinks; conventions/ still works everywhere |
| New machine | Clone repo; run setup script; update Machines table |
