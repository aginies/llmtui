# Cache Reuse

The **Cache Reuse** field controls how many tokens from the KV cache are reused when processing a new prompt that shares prefix context with the previous one.

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| **Cache Reuse** | 0 | Number of tokens to reuse from the previous KV cache |

## How It Works

When processing a new prompt that starts with the same text as the previous prompt, the cache reuse feature avoids recomputing the KV cache for the shared prefix. The number specified here is the maximum number of tokens that will be reused.

For example, if the previous prompt was "Hello, how are you today?" and the new prompt is "Hello, how are you today? What's the weather?", setting cache reuse to 8 would reuse the KV cache for the first 8 tokens ("Hello, how are you today") and only compute the new portion.

## When to Use

- **High values** (100+) — When processing many prompts with long shared prefixes (e.g., system prompts + user queries)
- **Low values** (0-16) — When prompts rarely share prefixes, or when you want to minimize memory usage
- **Zero** — Disables cache reuse entirely

## Config Key

`cache_reuse` — integer, default 0.
