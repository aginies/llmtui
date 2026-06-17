# Cache

KV cache management for performance optimization.

## Cache Prompt

Controls whether the KV cache is computed and stored for the prompt (input) tokens during generation.

### Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| **Cache Prompt** | true | Whether to cache the prompt tokens in the KV cache |

### How It Works

When enabled (default), the prompt tokens are cached in the KV cache during generation. This means:
- The prompt is only computed once, improving efficiency for repeated prompts
- The KV cache includes both prompt and generation tokens
- More KV cache memory is used

When disabled, the prompt tokens are not cached. This means:
- The prompt is recomputed on every iteration
- Less KV cache memory is used
- Useful for very long prompts where caching would exceed available memory

### When to Disable

- **Very long prompts** — When the prompt is so long that caching it would consume all available KV cache memory
- **Limited VRAM** — When you need to maximize memory for generation tokens
- **Streaming scenarios** — When processing prompts incrementally

### Config Key

`cache_prompt` — boolean, default true.

## Cache Reuse

Controls how many tokens from the KV cache are reused when processing a new prompt that shares prefix context with the previous one.

### Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| **Cache Reuse** | 0 | Number of tokens to reuse from the previous KV cache |

### How It Works

When processing a new prompt that starts with the same text as the previous prompt, the cache reuse feature avoids recomputing the KV cache for the shared prefix. The number specified here is the maximum number of tokens that will be reused.

For example, if the previous prompt was "Hello, how are you today?" and the new prompt is "Hello, how are you today? What's the weather?", setting cache reuse to 8 would reuse the KV cache for the first 8 tokens ("Hello, how are you today") and only compute the new portion.

### When to Use

- **High values** (100+) — When processing many prompts with long shared prefixes (e.g., system prompts + user queries)
- **Low values** (0-16) — When prompts rarely share prefixes, or when you want to minimize memory usage
- **Zero** — Disables cache reuse entirely

### Config Key

`cache_reuse` — integer, default 0.
