# Model Roles — Example Setup

> **This is a personal configuration example.**
> Copy this file to `model-roles.local.md` and customize it.
> `model-roles.local.md` is git-ignored and won't be overwritten by upstream updates.

## Machines

| Machine | OS | LLM Resources |
| --- | --- | --- |
| Windows PC | Windows 11 | Copilot (Sonnet 4.6 / GPT 4.1), Gemini CLI, Ollama (GPU) |
| Mac Mini | macOS | Copilot, Gemini CLI, Ollama (Apple Silicon) |

## Current Mapping

| Role | Machine A | Machine B |
| --- | --- | --- |
| PLANNER | Copilot (Claude Sonnet 4.6) | Async plan agent |
| CODER | Copilot Agent Mode | VS Code Copilot |
| TESTER | Gemini CLI (Gemini 2.5 Pro) | Gemini CLI |
| REVIEWER | Copilot (GPT 4.1) or Gemini CLI | Gemini CLI |
| LOCAL | Ollama: Breeze2-8B, TAIDE-LX-8B | Ollama: larger models |

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
