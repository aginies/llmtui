# Command Palette

The **Command Palette** is a fuzzy action finder for the TUI. Open it with `Ctrl+Space` (Normal mode only):

```
┌──────────────────────────────────────────────┐
│  COMMAND PALETTE                             │
│  Type to filter actions…                     │
│  > Focus Models panel              F1        │
│    Focus Server Settings panel     F2        │
│    Focus LLM Settings panel        F3        │
│    ...                                       │
│  ↵ Run    ⎋ Close                           │
└──────────────────────────────────────────────┘
```

## Keys

| Key | Action |
|-----|--------|
| Type | Filter actions (fuzzy subsequence match — `ks` matches "Kill llama-server") |
| `↑` / `↓` | Navigate matches (wraps around) |
| `Enter` | Run the selected action |
| `Esc` | Close |
| `Left` / `Right` / `Home` / `End` / `Backspace` / `Delete` | Edit the filter text |

Matching is a case-insensitive fuzzy subsequence: every character of the query must appear in order in the action name (or its extra search keywords), and word-start matches and consecutive runs score higher than scattered ones. An empty query lists all actions in registry order.

## Actions

| Action | Shortcut |
|--------|----------|
| Focus Models panel | `F1` |
| Focus Server Settings panel | `F2` |
| Focus LLM Settings panel | `F3` |
| Focus Log panel | `F6` |
| Focus Profiles panel | `Tab` |
| Focus System Prompt Presets panel | `Tab` |
| Search HuggingFace models | `/` |
| Show About dialog | `A` |
| Manage RPC workers | — |
| Show panel help | `Ctrl+H` |
| Show server command line | `Ctrl+K` |
| Show dashboard URL | `Ctrl+U` |
| Open profile picker | `Ctrl+P` |
| Cycle UI language | `Ctrl+L` |
| Replay onboarding wizard | `Ctrl+O` |
| Configure web search | — |
| Toggle expert mode | `Ctrl+X` |
| Save current settings | `Ctrl+S` |
| Exit llm-manager | `Ctrl+C` |

## Context-Dependent Actions

These entries appear only when relevant:

| Action | Shown when |
|--------|------------|
| Focus Downloads panel | A download is in progress |
| Explain GGUF filename | A model/file name is selected (same as `Ctrl+G`) |
| Cycle sort order | In list or search mode (same as `Ctrl+S`) |
| Kill llama-server | A server is running (same as `Ctrl+Alt+K`) |
