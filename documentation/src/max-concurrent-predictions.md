# Max Concurrent Predictions

The **Max Concurrent Predictions** field controls how many inference requests can be processed simultaneously by the llama.cpp server.

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| **Max Concurrent Predictions** | None (unlimited) | Maximum number of concurrent requests. `None` means no limit. |
| **Parallel** | 1 | Max concurrent predictions (alias for the above) |

## How It Works

When set to a specific number, the server limits concurrent inference to that many requests. This is useful for:
- Preventing VRAM exhaustion from too many simultaneous requests
- Controlling resource usage in multi-user environments
- Ensuring predictable latency under load

Set `None` (or 0) to allow unlimited concurrent predictions — the server handles requests as they arrive.

## Config Key

`max_concurrent_predictions` — integer or null. Also configurable via the `parallel` field in expert mode.
