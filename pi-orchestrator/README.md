# Pi Orchestrator

A Pi extension that gets reasoned answers from a **remote** llama.cpp server. The task is **always executed remotely** — the `orchestrate` tool sends the task (plus optional attached files) to the server's OpenAI-compatible API and returns its answer. Nothing runs locally.

## What it does

The `orchestrate` tool always runs the task on the remote llama.cpp server configured in `orchestrator.json` (or env vars). The agent only calls it when you explicitly ask for orchestration — include the keyword **`orchestrate`** or **`pi-orch`** in your prompt (e.g. "pi-orch review src/foo.ts"). Other requests are handled locally.

Two modes:

- **Sync (default)** — the task is sent to the remote server and the tool waits for the answer, returning it as the tool result.
- **Async (`async: true`)** — the request is fired in the background and the tool returns immediately. When the server responds, the answer is delivered to the session once the local session is idle.

Since the remote model has no file access, use `sendFiles` to attach the contents of specific files to the prompt.

## Flow

**Sync mode (default):**

```text
┌──────────────────┐     ┌───────────────────┐     ┌────────────┐
│  Pi Agent        │────▶│  llama.cpp server │────▶│  Final     │
│  (local LLM)     │     │  (remote, chat    │     │  Answer    │
│                  │     │   completion)     │     │            │
└──────────────────┘     └───────────────────┘     └────────────┘
```

**Async mode (`async: true`):** the tool returns `Task queued` immediately and the answer is delivered to the session once it is idle — see [Async Mode](#async-mode) for the full flow.

## Installation

Copy the extension into pi's extensions directory, then restart pi (or run `/reload`):

```bash
# Global — available in all projects
cp -r agent/extensions/orchestrator ~/.pi/agent/extensions/

# Or project-local
mkdir -p .pi/extensions
cp -r agent/extensions/orchestrator .pi/extensions/
```

Once loaded, the `orchestrate` tool and the `/orchestrator` commands are available in the session.

## Prerequisites

| Requirement | Details |
| ------------- | --------- |
| **Pi** | `pi` CLI installed and working |
| **llama.cpp server** | Required — the `orchestrate` tool always sends the task to it for the answer (typically on a remote host) |
| **Model** | A GGUF model loaded on the server |

## Usage

The `orchestrate(...)` examples below are **tool calls the agent makes inside a Pi session** — not shell commands you type in a terminal.

### Basic example (sync — default)

```text
orchestrate(
  task: "Explain the trade-offs between REST and gRPC for an internal service",
  llamaModel: "qwen3.8",
  llamaSystemPrompt: "You are an API architect."
)
```

The task goes to the remote llama.cpp server; the answer comes back as the tool result.

### Code review with attached files (sendFiles)

The remote model has no file access — use `sendFiles` to append the full contents of specific files to the prompt so it can review the actual code:

```text
orchestrate(
  task: "Review the attached code for quality issues, potential bugs, and improvement suggestions",
  sendFiles: ["agent/extensions/orchestrator/index.ts"],
  llamaSystemPrompt: "You are a senior code reviewer. Review the code in the Attached Files section."
)
```

### Whole-project review (sendDir — transfer mode)

For whole-project reviews, don't stuff files into the prompt — ship the project to the remote server and let its review agent read the files itself. `sendDir` (default: `cwd`) is tarred (excluding `.git`, `node_modules`, `target`, `dist`, `build`, `.venv`, `__pycache__`), uploaded to the remote llm-manager's transfer API, and its agent runs over the extracted files. **No prompt-size limit** — the remote model pages through the files with `LIST`/`READ`/`GREP` tools.

```text
orchestrate(
  task: "Review this project for bugs and style issues",
  sendDir: ".",
  sendDiff: true          // optional: the diff is included in the task text
)
```

Requirements on the remote server: llm-manager with `api_endpoint_enabled` **and** `api_transfer_enabled` set to `true`, and its `api_endpoint_key` configured as `llamaApiKey` here. The tool reports the transfer id, round count, and files read in the result.

### Multi-file analysis

```text
orchestrate(
  task: "Summarize the config layering strategy in the attached files and identify any issues.",
  sendFiles: ["orchestrator.json", "agent/extensions/orchestrator/index.ts"],
  llamaModel: "qwen3.8"
)
```

### Resolving attached files from a different directory

Relative `sendFiles` paths resolve against `cwd`:

```text
orchestrate(
  task: "Check the attached config for issues",
  sendFiles: ["orchestrator.json"],
  cwd: "/home/aginies/devel/github/aginies/securemark"
)
```

### Parameters

| Parameter | Type | Description |
| --------- | ---- | ----------- |
| `task` | `string` | The task to run (required) — always executed on the remote server |
| `sendDir` | `string` | **Transfer mode**: tar this directory (default `cwd`), upload it to the remote transfer API, and run the remote review agent over the files. No prompt-size limit. Requires `llamaApiKey` |
| `sendFiles` | `string[]` | File paths to attach — contents appended to the prompt so the remote model can see the code (relative paths resolve against `cwd`). Line ranges supported: `"src/foo.ts:10-120"` or `"src/foo.ts:42"` |
| `sendDiff` | `boolean \| "unstaged" \| "staged" \| "all"` | Attach the current git diff: `true` = unstaged changes, or `"staged"` / `"all"` (staged + unstaged vs HEAD). Default `false` |
| `async` | `boolean` | `true` to fire the request in the background and return immediately. Default `false`: wait for the remote answer and return it as the tool result |
| `llamaUrl` | `string` | Remote llama.cpp server URL |
| `llamaModel` | `string` | Model name on the remote server |
| `llamaSystemPrompt` | `string` | System prompt for the remote request (inline mode only — in transfer mode the remote agent uses its own prompt) |
| `llamaApiKey` | `string` | Bearer API key for the remote llm-manager — required for `sendDir` transfer mode |
| `llamaMaxTokens` | `number` | Max tokens for the remote response |
| `llamaTemperature` | `number` | Temperature for the remote request |
| `llamaTopP` | `number` | Top-p for the remote request |
| `cwd` | `string` | Base directory for resolving relative `sendFiles` paths |

All parameters apply to both sync and async modes.

## Configuration

Config is merged in this priority order (later layers override earlier ones):

```text
hardcoded defaults < env vars < ~/.pi/orchestrator.json < .pi/orchestrator.json
```

### Environment Variables

```bash
export ORCHESTRATOR_LLAMA_URL="http://remote-host:8080"
export ORCHESTRATOR_LLAMA_MODEL="qwen3.8"
export ORCHESTRATOR_LLAMA_SYSTEM_PROMPT="You are a code reviewer."
export ORCHESTRATOR_LLAMA_API_KEY="your-secret-key"   # required for sendDir transfer mode
export ORCHESTRATOR_LLAMA_MAX_TOKENS=4096
export ORCHESTRATOR_LLAMA_TEMPERATURE=0.7
export ORCHESTRATOR_LLAMA_TOP_P=0.9
export ORCHESTRATOR_MAX_PROMPT_LENGTH=65536
export ORCHESTRATOR_MAX_FILE_CONTENT=32768
export ORCHESTRATOR_DIFF_MAX_SIZE=32768
```

### Project Config (`.pi/orchestrator.json`)

```json
{
  "llamaUrl": "http://remote-host:8080",
  "llamaModel": "qwen3.8",
  "llamaSystemPrompt": "You are a code reviewer. Analyze the following for quality issues, potential bugs, and improvement suggestions.",
  "llamaApiKey": "your-secret-key",
  "llamaMaxTokens": 4096,
  "llamaTemperature": 0.7,
  "llamaTopP": 0.9,
  "maxPromptLength": 65536,
  "maxFileContent": 32768,
  "diffMaxSize": 32768
}
```

### Prompt size limits

| Key | Default | Meaning |
| --- | ------- | ------- |
| `maxPromptLength` | `65536` (64 KB) | Hard cap on the total user message sent to the server. Prevents OOM on small contexts. |
| `maxFileContent` | `32768` (32 KB) | Per-file cap for `sendFiles` attachments. |
| `diffMaxSize` | `32768` (32 KB) | Cap for the `sendDiff` attachment. |

Size them to your server's context window (`-c` on `llama-server`): 128k tokens ≈ 500 KB of text, so e.g. `maxPromptLength: 393216` (384 KB ≈ 96k tokens) leaves headroom for the system prompt and the model's own `llamaMaxTokens` output. The defaults are conservative on purpose — raise them only if your context can hold it.

### Attachment budget allocator

Attached files are allocated against `maxPromptLength` (minus the task text) **in the given order** — nothing is cut silently:

1. **Whole files while they fit** (each still capped at `maxFileContent`)
2. **Next file truncated** to the remaining budget — but only if at least 4 KB of it fits; the cut is marked inline (`... [truncated: 18.2 KB of 40.9 KB]`)
3. **The rest dropped**

If anything was truncated or dropped, the prompt ends with a manifest so both you and the remote model know exactly what's missing:

```text
[attachments: 1 truncated (src/big.ts 18.2 KB/40.9 KB), 2 dropped (src/huge1.ts, src/huge2.ts); 55.1 KB over budget]
```

Practical consequence: put the files that matter most **first** in `sendFiles` — order is the priority. The old behavior (per-file caps, then the whole prompt silently sliced at `maxPromptLength` mid-file) is gone; the request-level cap in `chatCompletion` remains only as a last-resort safety net.

### Git diff attachment

For "review my changes", the most natural thing to send is the diff itself — not whole files:

```text
orchestrate(
  task: "Review my uncommitted changes for bugs and style issues",
  sendDiff: true            // unstaged changes
)
```

| Value | Command | Sends |
| ----- | ------- | ----- |
| `true` | `git diff` | unstaged changes |
| `"staged"` | `git diff --staged` | staged changes |
| `"all"` | `git diff HEAD` | staged + unstaged vs HEAD |

- The diff is appended under a `--- Changes (git diff: unstaged) ---` section, between the task and any `sendFiles` attachments
- Capped at `diffMaxSize` (default 32 KB, `ORCHESTRATOR_DIFF_MAX_SIZE`); the diff counts against the prompt budget, so attachments are allocated around it
- Degrades to an inline note instead of failing: `(no unstaged changes)`, `(git diff failed: not a git repository)`, etc.
- Composes with `includeContext`-style workflows: diff + a couple of line-range attachments of the affected files gives the remote model both the change and its surroundings

### Line-range attachments

You don't have to attach whole files — append a 1-based, inclusive line range to any `sendFiles` entry:

```text
sendFiles: [
  "src/foo.ts:10-120",   // lines 10 through 120
  "src/bar.ts:42"        // just line 42
]
```

- Only the selected lines are sent — they count against the budget at their actual (smaller) size, so a range of a huge file is cheap
- The attachment header shows what was selected: `### src/foo.ts (lines 10–120 of 340)`
- Ranges are clamped to the file's length and the clamping is noted: `### src/foo.ts (lines 95–100 of 100, requested 95–200)`
- A range with no overlap is noted instead of failing: `(no lines: requested 500–600, file has 100 lines)`
- Reversed ranges (`120-10`) are silently normalized
- Plain paths (no `:lines`) work exactly as before

### User Config (`~/.pi/orchestrator.json`)

Same schema as project config, applies across all projects.

## Tool Output Format

The remote llama.cpp answer is returned as-is (sync mode) or delivered as a `CustomMessage` (async mode).

## When to use

- **Sync (default)** — self-contained questions, analysis of text you provide, or code review with `sendFiles` attachments
- **Background tasks** — use `async: true` to run long remote tasks without blocking the current session

## Async Mode

The `orchestrate` tool supports an **async** mode (`async: true`) that fires the remote request in the background and returns immediately. This lets you continue working while the server processes the task.

### Basic usage

```text
orchestrate(
  task: "Run a full security audit of the attached code",
  sendFiles: ["src/auth.ts", "src/api.ts"],
  llamaModel: "qwen3.8",
  async: true
)
```

The tool immediately returns:

```text
⏳ Task queued (id: ab12...) — result will be delivered when done
```

The agent can then continue with other tasks (code edits, reading files, etc.). When the remote server responds:

1. The result is emitted via the EventBus
2. The session listener waits for the session to be idle (polls every 500ms, max 10 min)
3. The result is delivered as a `CustomMessage` and rendered inline in the session

### Example workflow

```text
# Start a long-running analysis in the background
orchestrate(
  task: "Identify type safety issues in the attached files",
  sendFiles: ["src/a.ts", "src/b.ts", "src/c.ts"],
  llamaModel: "qwen3.8",
  async: true
)

# Meanwhile, the agent can do other work:
# - Edit files
# - Run tests
# - Answer other questions

# When the remote server responds, the answer appears in the session as:
# orchestrator: qwen3.8
# [remote answer]
```

### How it works

```text
┌──────────────────┐     ┌───────────────────┐     ┌──────────────┐     ┌──────────────┐
│  Pi Agent        │────▶│  llama.cpp server │────▶│  EventBus    │────▶│  CustomMsg   │
│  (local LLM)     │     │  (remote, in      │     │  listener    │     │  (delivered  │
│  returns "Task   │     │   background)     │     │  waits for   │     │  when idle)  │
│  queued"         │     └───────────────────┘     │  idle        │     └──────────────┘
└──────────────────┘                               └──────────────┘
       │                       │                        │                        │
       │  async: true          │  background            │  event bus           │  idle
       │  returns immediately  │  request               │  listener            │  delivers
       ▼                       ▼                        ▼                        ▼
  "Task queued"       Remote answer             CustomMessage
  (agent continues)   rendered in session
```

### Notes

- The async task **survives the tool call** — it is not killed when the tool returns
- If the remote request fails, the error is delivered to the session as a `✗ orchestrator: …` message
- The idle-wait timeout is 10 minutes to prevent forever-waiting in case of a crash
- The result is delivered with `deliverAs: "followUp"` so it queues behind any active streaming

### Enabling / Disabling

`/orchestrator enable` and `/orchestrator disable` activate or deactivate the `orchestrate` tool for the session:

```text
/orchestrator disable
→ orchestrator disabled — orchestrate tool hidden (persists across sessions)

/orchestrator enable
→ orchestrator enabled — orchestrate tool active
```

- **Disabled** removes `orchestrate` from the LLM's tool list — the model can no longer see or call it. The `/orchestrator` commands themselves stay available so you can re-enable at any time
- **Enabled** puts `orchestrate` back in the active tool list
- The state is **persisted** as the `enabled` key in `~/.pi/orchestrator.json` (global, same file as the rest of the orchestrator config), so a disabled orchestrator stays disabled after pi restarts until you run `/orchestrator enable`
- `/orchestrator status` shows the current state (`enabled` / `disabled`) in its header
- Default is **enabled** — a missing or corrupt state file means the tool is active

### Listing background tasks

`/orchestrator` commands are slash commands typed in the **Pi session chat** (not terminal commands). Type `/orchestrator status` to see all tracked background tasks:

```text
orchestrator: 1 running, 4 total · enabled
⏳ ab12cd34    42s+  qwen3.8  Run a full security audit of the attached code
✓  9f8e7d6c     128s  qwen3.8  Summarize the config layering strategy
✗  1a2b3c4d      9s  qwen3.8  Check the database migration
    llama.cpp request failed: …
⊘  5e6f7a8b      30s  qwen3.8  Audit the attached code for security issues
    Killed by user
```

- `⏳` running · `✓` done · `✗` failed · `⊘` killed (first error line shown)
- The list is TUI-only — it does not enter the LLM context
- In-memory only, capped at the 50 most recent tasks; it resets when pi restarts

### Killing a task

`/orchestrator kill <id>` (id can be any unique prefix, autocomplete helps) aborts a running background task:

```text
/orchestrator kill ab12
```

To kill **all** running tasks at once, use `all`:

```text
/orchestrator kill all
```

- The in-flight request(s) to the remote server are aborted
- The task(s) show as `⊘ killed` in `/orchestrator status` and a `Task killed by user` note is delivered to the session
- Only `running` tasks can be killed; finished ones are left as-is

### Health check

`/orchestrator ping` (optionally `/orchestrator ping <url>` to check a different server) hits the llama.cpp `/health` endpoint with a 5s timeout:

```text
/orchestrator ping
→ ✓ http://remote-host:8080 ok (12ms)

/orchestrator ping http://other-host:9090
→ ✗ http://other-host:9090 unreachable: timeout after 5s
```

The result is shown as a notification and in the status bar. The configured URL is also pinged automatically as a **pre-flight before the first `orchestrate` call** in a session (and again whenever the target URL changes) — if the server is down, the call fails fast with a clear `llama.cpp server unreachable at …` error instead of hanging for the full 300s request timeout.

## Remote Execution & File Access

The task is **always executed on the remote llama.cpp server** (`llamaUrl`) — there is no local execution path.

- The configured `llamaSystemPrompt` still applies (override per call, or pass `""` to skip)
- The remote model only sees the task text — **no file access, no tools**
- **`sendFiles`** appends the contents of specific files to the prompt under an `--- Attached Files ---` section, so the remote model can review actual code. Each file is capped at `maxFileContent` (default 32 KB) and the set is allocated against `maxPromptLength` (default 64 KB) in the given order — see [Attachment budget allocator](#attachment-budget-allocator). Missing or binary files are noted inline instead of failing the call.

## System Prompt (opt-in)

The llama.cpp system prompt is **opt-in** — it is empty by default. No persona or instructions are injected unless you explicitly configure one.

### Bypassing

If a config file or env var sets a system prompt and you want to override it for a single call, pass an empty string:

```text
orchestrate(
  task: "Summarize the config layering strategy",
  llamaSystemPrompt: ""
)
```

This tells the orchestrator to send the request without a system message — the user message (task + attached files) goes to the model unmodified.

### Configuring

Set the system prompt via any of these methods (merged in priority order):

**Environment variable:**

```bash
export ORCHESTRATOR_LLAMA_SYSTEM_PROMPT="You are a code reviewer."
```

**Project config (`.pi/orchestrator.json`):**

```json
{
  "llamaSystemPrompt": "You are a code reviewer. Focus on architecture and edge cases."
}
```

**User config (`~/.pi/orchestrator.json`):**

```json
{
  "llamaSystemPrompt": "You are a code reviewer."
}
```

All three can be overridden per-call by passing `llamaSystemPrompt` directly to `orchestrate()`.

## Troubleshooting

| Problem | Fix |
| --------- | ----- |
| `llama.cpp request failed` | Check the remote server is running and reachable: `curl http://remote-host:8080/health` |
| `timeout` / `abort` | Remote requests time out after a fixed 300s (hardcoded) — reduce task complexity or split it across multiple calls |
| Output truncated | Files are budgeted against `maxPromptLength` (default 64 KB, configurable) in `sendFiles` order — the manifest line in the prompt says what was truncated/dropped. Put important files first, or raise the cap |

## Example llama.cpp setup

```bash
# Start llama.cpp server with a model (on the remote host)
llama-server -m models/qwen3.8.Q4_K_M.gguf --port 8080

# Test connectivity
curl http://remote-host:8080/v1/models
```

Then, in a Pi session, the agent calls the tool (tool call, not shell):

```text
orchestrate(task: "Summarize the key findings from the security audit", llamaModel: "qwen3.8")

# Or with attached files for code review
orchestrate(task: "Audit the attached code for security issues", sendFiles: ["src/auth.ts"], llamaModel: "qwen3.8")
```
