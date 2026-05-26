# WebSocket Dashboard

The WebSocket Dashboard provides a real-time visualization of model metrics and settings via a web browser.

## Accessing the Dashboard

The dashboard runs as a built-in HTTP server on port **49223** by default. Open it in your browser:

```
http://localhost:49223
```

## Configuration

To enable and configure the dashboard:

1. Open the **Server Settings** panel (F2)
2. Navigate to **Dashboard** and press `Enter`
3. Configure:
   - **Enabled** — toggle on/off
   - **Port** — server port (default: 49223)
   - **Auth Key** — optional authentication (see below)
4. Press `Enter` to save, `Esc` to close

## Authentication

When an auth key is configured, clients must include it as a query parameter:

```
http://localhost:49223?auth=mysecretkey
```

## Metrics Displayed

The dashboard shows real-time metrics from the llama.cpp server:

| Metric | Description |
|--------|-------------|
| Generation Speed | Tokens per second (TPS) |
| Prompt Speed | Prompt processing TPS |
| Latency | Milliseconds per token |
| Context | Context window usage (progress bar) |
| VRAM | GPU memory used/total (progress bar) |
| RAM | System memory usage |
| CPU | CPU usage percentage |

## Settings Panel

The dashboard also displays current inference settings:

- Backend and llama.cpp version
- Threads, threads_batch, context_length, batch_size, ubatch_size
- Sampling parameters: temperature, top_k, top_p, min_p, typical_p, seed
- Repetition control: repeat_penalty, repeat_last_n, presence_penalty, frequency_penalty
- Mirostat mode and parameters
- Flash Attention, KV Cache Offload, Cache Type K/V
- Unified KV, Mlock, Mmap, Embedding, Jinja, Ignore EOS
- Expert Count, GPU Layers, Samplers

## Server Command

The full llama-server command line is displayed at the bottom of the dashboard, showing the exact invocation with all parameters.

## Architecture

The dashboard server is built with `axum` and `tokio`. It:

1. Creates a `broadcast::channel(64)` for metrics distribution
2. Spawns the server on the configured port
3. Each metrics update is sent to the broadcast channel
4. WebSocket clients subscribe and receive real-time updates
5. The HTML dashboard (embedded in the binary) connects via WebSocket and renders the metrics

The server is started/stopped automatically when you toggle the Dashboard setting in Server Settings.
