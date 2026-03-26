# Model Roles

Define roles by **what they do**, not by which model they are.
When you switch AI tools, update the mapping table — everything else stays the same.

## Roles

| Role | Purpose | Key Trait |
| :--- | :--- | :--- |
| PLANNER | Analyze requirements, produce plan files | Broad reasoning, architecture awareness |
| ARCHITECT | Adversarial plan review — trade-offs, over-engineering, bugs | Critical thinking, minimalism, direct communication |
| ANALYST | Business logic review — ROI, domain correctness, user impact | Commercial awareness, domain expertise |
| CODER | Write implementation code following a plan | Code generation, refactoring |
| TESTER | Write tests from plan spec + public API only | Spec-driven, does NOT read implementation |
| REVIEWER | Review code for bugs, security, style | Critical eye, different perspective |
| SCRIBE | End-of-day diary, shutdown enforcement | Summarization, Obsidian integration |
| LOCAL | Tasks requiring privacy or local language | Runs on-device, no data leaves machine |

## Routing Rules

1. **CODER and TESTER must be different models** — independent verification
2. **REVIEWER should differ from CODER** — fresh perspective catches blind spots
3. **LOCAL** is for privacy-sensitive data or Traditional Chinese tasks
4. When switching tools, update the **Current Mapping** table below only

## Machines

| Machine | OS | LLM Resources |
| :--- | :--- | :--- |
| Windows PC | Windows 11 | Copilot (Sonnet 4.6 / GPT 4.1), Gemini CLI, Ollama (3060 Ti 12GB) |
| Mac Mini | macOS | OpenClaw (Gemini), Copilot, Gemini CLI, Ollama (Apple Silicon) |

## Current Mapping (2026-03)

| Role | Windows PC | Mac Mini |
| :--- | :--- | :--- |
| PLANNER | Copilot (Claude Sonnet 4.6) — interactive | OpenClaw Plan Agent — async via Telegram |
| CODER | Copilot Agent Mode (Claude Sonnet 4.6) | VS Code Copilot (for iOS/Swift work) |
| TESTER | Gemini CLI (Gemini 2.5 Pro) | Gemini CLI |
| REVIEWER | Copilot (GPT 4.1) or Gemini CLI | OpenClaw (Gemini) |
| LOCAL | Ollama: Breeze2-8B, TAIDE-LX-8B | Ollama: larger models on Apple Silicon |

## Ollama Models Available

### Windows PC (RTX 3060 Ti, 12GB VRAM)

| Model | Best For |
| :--- | :--- |
| `willqiu/Llama-Breeze2-8B-Instruct` | Traditional Chinese tasks |
| `TAIDE-LX-8B` | Traditional Chinese, Taiwan-specific |
| `gemma3:latest` | General, multilingual |
| `mistral:latest` | Fast general purpose |
| `TwinkleAI/Llama-3.2-3B-F1-Reasoning` | Quick reasoning |
| `cwchang/llama-3-taiwan-8b-instruct` | Taiwan-specific |

### Mac Mini (Apple Silicon — TBD)

Models to be configured after Mac Mini arrives. Apple Silicon unified memory
allows running larger models (e.g., 70B quantized) that don't fit in 12GB VRAM.

## Typical Workflow (Single Developer)

```text
1. PLAN    → Copilot (Sonnet 4.6) in VS Code
             OR Mac Mini Plan Agent via Telegram (async, on the go)

2. IMPLEMENT → Copilot Agent Mode (Sonnet 4.6) in VS Code
               Follow the approved plan, check off items

3. TEST    → Open a second terminal, use Gemini CLI
             Feed it: plan file + public interfaces only
             It writes tests without seeing implementation

4. REVIEW  → Switch Copilot to GPT 4.1 (different model)
             OR use Gemini CLI with cross-review template
             Check: bugs, security, architecture violations

5. VERIFY  → Run full test suite: `dotnet test` / `cargo test` / `npm test`
             Confirm all plan items implemented
             Update .dev/state.md
```

## Migration Examples

When you switch to a new tool, update this table:

| Scenario | What Changes |
| :--- | :--- |
| Adopt Claude Code | CODER → Claude Code; run `Sync-DevContext` to generate CLAUDE.md |
| Adopt OmO/OpenCode | All roles → OmO discipline agents; AGENTS.md already generated |
| Adopt Antigravity | Update table; add adapter in sync script |
| Better local model | LOCAL → Ollama (new model); no other changes |
| Drop Copilot entirely | Remove copilot-skills/ symlinks; conventions/ still works everywhere |
| New machine | Clone dotdev; run setup script; update Machines table |
