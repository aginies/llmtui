# Pi Orchestrator

**pi-orchestrator** is a [Pi](https://github.com/badlogic/pi-mono) coding-agent extension shipped in this
repository (`pi-orchestrator/`). It gives a Pi session an `orchestrate` tool that sends a task — and
optionally files — to a **remote** llama.cpp server managed by llm-manager and returns the model's
answer. Nothing runs locally: the local Pi agent only orchestrates, the remote model does the work.

Typical use: keep a small fast model in your local session, and offload heavy reasoning ("review this
project", "run a security audit") to a bigger model on a remote GPU box.

> Full reference (all parameters, async mode, attachment budget, output format):
> [`pi-orchestrator/README.md`](https://github.com/aginies/llmtui/blob/main/pi-orchestrator/README.md)

## How it works

```text
┌──────────────────┐     ┌───────────────────┐     ┌────────────┐
│  Pi Agent        │────▶│  llama.cpp server │────▶│  Final     │
│  (local LLM)     │     │  (remote, chat    │     │  Answer    │
│                  │     │   completion)     │     │            │
└──────────────────┘     └───────────────────┘     └────────────┘
```

- **Sync (default)** — the tool waits for the remote answer and returns it as the tool result. The
  remote output streams live into the tool view while it generates.
- **Async (`async: true`)** — the request is fired in the background and the tool returns immediately;
  the answer is delivered to the session once it is idle.
- **Transfer mode (`sendDir`)** — the directory is tarred, uploaded to the remote llm-manager's
  File Transfer API, and the remote *review agent* pages through the files itself (`LIST`/`READ`/
  `GREP`). No prompt-size limit, even for whole projects.
- **Auto file transfer** — if the task text mentions existing file paths (e.g. "review `src/foo.ts`"),
  those files are sent via the transfer API automatically (skipped when `sendFiles`/`sendDiff`/
  `sendDir` are passed explicitly).

The local agent only calls the tool when you ask for orchestration — include the keyword
**`orchestrate`** or **`pi-orch`** in your prompt (e.g. "pi-orch review src/foo.ts").

## Installation

Copy the extension into pi's extensions directory, then restart pi (or run `/reload`):

```bash
# Global — available in all projects
cp -r pi-orchestrator/agent/extensions/orchestrator ~/.pi/agent/extensions/

# Or project-local
mkdir -p .pi/extensions
cp -r pi-orchestrator/agent/extensions/orchestrator .pi/extensions/
```

Once loaded, the `orchestrate` tool and the `/orchestrator` slash commands are available.

## Remote server requirements

The remote machine must run llm-manager with:

| Setting | Value | Purpose |
| ------- | ----- | ------- |
| `api_endpoint_enabled` | `true` | Serves the OpenAI-compatible API proxy |
| `api_transfer_enabled` | `true` | Serves the File Transfer API (needed for `sendDir` / auto transfer) |
| `api_endpoint_key` | any secret | Bearer key — set it as `llamaApiKey` in the orchestrator config |

Plain chat (no `sendDir`) only needs `api_endpoint_enabled` — the tool can also point directly at a
raw `llama-server` URL, which doesn't need a key at all.

## Configuration

Config is merged in this priority order (later layers override earlier ones):

```text
hardcoded defaults < env vars < ~/.pi/orchestrator.json < .pi/orchestrator.json
```

### Example user config (`~/.pi/orchestrator.json`)

```json
{
  "llamaUrl": "http://remote-host:8080",
  "llamaModel": "qwen3.8",
  "llamaSystemPrompt": "You are a code reviewer.",
  "llamaApiKey": "secret",
  "llamaMaxTokens": 64000,
  "llamaTemperature": 0.7,
  "llamaTopP": 0.9,
  "maxPromptLength": 393216,
  "maxFileContent": 131072
}
```

- `llamaApiKey` — Bearer key of the remote llm-manager (required for `sendDir` / auto transfer)
- `llamaMaxTokens` — max tokens for the remote response; raise it for thinking models
  (e.g. Qwen3): reasoning tokens count against this budget, and a model that exhausts it
  while thinking returns no final answer
- `maxPromptLength` — total prompt budget; `393216` (384 KB ≈ 96k tokens) suits a 128k-context server
- `maxFileContent` — per-file cap for `sendFiles` attachments; `131072` (128 KB) for large files

The same file (or the project-level `.pi/orchestrator.json`) also carries the `enabled` key written
by `/orchestrator enable|disable`.

### Environment variables

```bash
export ORCHESTRATOR_LLAMA_URL="http://remote-host:8080"
export ORCHESTRATOR_LLAMA_MODEL="qwen3.8"
export ORCHESTRATOR_LLAMA_SYSTEM_PROMPT="You are a code reviewer."
export ORCHESTRATOR_LLAMA_API_KEY="your-secret-key"   # required for sendDir transfer mode
export ORCHESTRATOR_LLAMA_MAX_TOKENS=16384
export ORCHESTRATOR_LLAMA_TEMPERATURE=0.7
export ORCHESTRATOR_LLAMA_TOP_P=0.9
export ORCHESTRATOR_MAX_PROMPT_LENGTH=65536
export ORCHESTRATOR_MAX_FILE_CONTENT=32768
export ORCHESTRATOR_DIFF_MAX_SIZE=32768
```

### Prompt size limits

| Key | Default | Meaning |
| --- | ------- | ------- |
| `maxPromptLength` | `65536` (64 KB) | Hard cap on the total user message sent to the server |
| `maxFileContent` | `32768` (32 KB) | Per-file cap for `sendFiles` attachments |
| `diffMaxSize` | `32768` (32 KB) | Cap for the `sendDiff` attachment |

Size them to the server's context window (`-c` on `llama-server`): 128k tokens ≈ 500 KB of text.
The defaults are conservative on purpose — raise them only if your context can hold it.

## Usage

These are **tool calls the agent makes inside a Pi session** — not shell commands.

```text
# Basic question
orchestrate(task: "Explain the trade-offs between REST and gRPC for an internal service")

# Code review with attached files (line ranges supported: "src/foo.ts:10-120")
orchestrate(
  task: "Review the attached code for bugs and style issues",
  sendFiles: ["src/foo.ts", "src/bar.ts"]
)

# Review my uncommitted changes
orchestrate(task: "Review my uncommitted changes", sendDiff: true)

# Whole-project review — no prompt-size limit
orchestrate(task: "Review this project for bugs", sendDir: ".")

# Long task in the background
orchestrate(task: "Run a full security audit", sendFiles: ["src/auth.ts"], async: true)
```

### Slash commands

| Command | Effect |
| ------- | ------ |
| `/orchestrator status` | List background tasks (with enabled/disabled state) |
| `/orchestrator enable` · `/orchestrator disable` | Show/hide the `orchestrate` tool (persisted in `~/.pi/orchestrator.json`) |
| `/orchestrator kill <id\|all>` | Abort a running background task (or all) |
| `/orchestrator ping [url]` | Health-check the remote server |

The configured URL is also pinged automatically before the first `orchestrate` call in a session, so
a dead server fails fast instead of hanging.

## When to use what

| Need | Mode |
| ---- | ---- |
| Self-contained question / analysis of pasted text | Sync, no attachments |
| Review specific files | Sync + `sendFiles` (line ranges for huge files) |
| Review uncommitted changes | Sync + `sendDiff` |
| Whole-project review | `sendDir` (transfer mode) |
| Long task without blocking the session | `async: true` |
