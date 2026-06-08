# Cache Prompt

The **Cache Prompt** field controls whether the KV cache is computed and stored for the prompt (input) tokens during generation.

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| **Cache Prompt** | true | Whether to cache the prompt tokens in the KV cache |

## How It Works

When enabled (default), the prompt tokens are cached in the KV cache during generation. This means:
- The prompt is only computed once, improving efficiency for repeated prompts
- The KV cache includes both prompt and generation tokens
- More KV cache memory is used

When disabled, the prompt tokens are not cached. This means:
- The prompt is recomputed on every iteration
- Less KV cache memory is used
- Useful for very long prompts where caching would exceed available memory

## When to Disable

- **Very long prompts** — When the prompt is so long that caching it would consume all available KV cache memory
- **Limited VRAM** — When you need to maximize memory for generation tokens
- **Streaming scenarios** — When processing prompts incrementally

## Config Key

`cache_prompt` — boolean, default true.
