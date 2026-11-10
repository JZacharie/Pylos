---
name: auto-deepseek-delegation
description: >-
  Automatically generate lightweight helper skills based on user instructions to reduce context size and token usage. For tasks identified as complex (e.g., > 200 lines of code generation, heavy refactoring, or when token budget would exceed a safe threshold), this skill transparently forwards the work to the `deepseek-coder` agent.
---

# Overview
This skill provides a two‑step workflow:
1. **Skill scaffolding** – Given a concise natural‑language description, it creates a new skill directory under `<repo>/.agents/skills/<skill_name>/` (or `<repo>/.gemini/skills/<skill_name>/`) containing a `SKILL.md` that documents the intent and any helper scripts.
2. **Complexity detection** – When the user request exceeds a configurable token budget (default ≈ 200 tokens) or explicitly mentions heavy code generation, the skill automatically invokes the `deepseek-coder` skill to perform the heavy lifting.

# Usage
- **Trigger**: Call this skill with a JSON payload containing `instruction` (the user‑provided description) and optional `complexity_hint`.
- **Output**: The skill writes a new folder and `SKILL.md` file, then returns the path of the created skill. If the request is complex, it also forwards the work to `deepseek-coder` and returns the result.

# Parameters
```json
{
  "instruction": "string",        // Human readable description of the desired skill.
  "complexity_hint": "optional string" // e.g., "generate 300 lines", "refactor entire crate".
}
```

# Implementation Details
- **Token budget check** – Count words in `instruction`; if > 30 words or `complexity_hint` contains keywords like `generate`, `refactor`, `massive`, treat as complex.
- **Skill creation** – Sanitize the instruction to a snake_case folder name, create the directory, and write a templated `SKILL.md` containing the instruction.
- **DeepSeek delegation** – If complex, call the existing `deepseek-coder` skill with the same payload. The result is stored under `output.txt` in the new skill folder.

# Security & Safety
- No external network calls are performed by this skill; it operates purely on the filesystem.
- The generated skill files are limited to the repository root (`<repo>`).
- All generated code is wrapped in a minimal template; the user must review before applying.

# Example Invocation
```json
{
  "instruction": "Add a utility to format dates in the UI and expose it via a new hook.",
  "complexity_hint": "generate 150 lines of TypeScript"
}
```
The skill will:
1. Create `.agents/skills/format_dates_hook/`.
2. Write a `SKILL.md` describing the new hook.
3. Detect complexity and forward the heavy generation to `deepseek-coder`, saving the result to `output.txt`.
