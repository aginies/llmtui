# Benchmark Tuning

Benchmark Tuning finds the optimal settings for your model and hardware by automatically testing multiple parameter combinations and measuring performance.

## When to Benchmark

Benchmark before deploying a model in production to:
- Find the fastest settings for your specific GPU/CPU
- Compare tradeoffs between throughput (TPS) and latency
- Determine the best context length for your use case
- Validate speculative decoding improvements
- Compare different backends (CPU vs Vulkan vs ROCm vs CUDA)

## Accessing Benchmark Mode

Set the server **Mode** to `BenchTune` in Server Settings, then press `Enter` to open the BenchTune Setup modal.

## Benchmark Modes

Two modes are available, each with different tradeoffs:

### RuntimeOnly (Recommended)

Single server, all parameters sent in the request body. No server restarts between tests.

- **Pros:** Fast (seconds per test), low overhead, preserves server state
- **Cons:** Some parameters may not be reconfigurable at runtime
- **Best for:** Sampling parameters (temperature, top-k, top-p), repetition control, DRY settings

### Full

Spawns a new server for each parameter combination.

- **Pros:** Tests all parameters including server-level settings (threads, context, GPU layers)
- **Cons:** Slow (minutes per test due to server startup), higher resource usage
- **Best for:** Hardware-level parameters, backend selection, architecture tuning

## Tunable Parameters

The following parameters can be tuned. Enable/disable each with `Space`:

| Parameter | Range | Description | Server/Client |
|-----------|-------|-------------|---------------|
| **Temperature** | 0.4–1.0 | Sampling randomness | Both |
| **Top-p** | 0.8–1.0 | Nucleus sampling threshold | Both |
| **Top-k** | 10–40 | Token sampling window | Both |
| **Repeat Penalty** | 1.0–1.5 | Repetition suppression | Both |
| **Flash Attention** | 0/1 | Enable/disable Flash Attention 2 | Server |
| **Threads** | 4–16 | CPU threads for generation | Server |
| **Batch Size** | 512–2048 | Logical maximum batch size | Server |
| **Expert Count** | -1–4 | MoE experts per token (MoE models) | Server |
| **Context Length** | Model default–max | Context window size | Server |
| **Spec Type** | draft-mtp, ngram-simple, etc. | Speculative decoding method | Server |
| **Draft Tokens** | 0–8 | Draft tokens per step (speculative) | Server |

### Server vs Client Parameters

- **Server parameters** require a full server restart to change (threads, context, flash attention). Use `Full` mode.
- **Client parameters** can be changed per-request (temperature, top-p, top-k). Use `RuntimeOnly` mode.

## Benchmark Configuration

### Prompt

The prompt used for each test iteration. Default:

```
Create Mona Lisa image in ascii art using text, number, symbol, everything possible. this should be the perfect painting.
```

Edit with `Alt+P`. Use a prompt representative of your actual workload for meaningful results.

### n-predict (Max Tokens)

Number of tokens to generate per test. Default: 512.

Edit with `Alt+N`. Higher values give more stable TPS measurements but take longer.

### Iterations

Number of test iterations per parameter combination. Default: 3.

Edit with `Alt+I`. More iterations reduce variance but increase total benchmark time.

### Test Duration

Maximum time per test iteration. Default: 30 seconds.

### Test Timeout

Maximum time for the entire benchmark run. Default: 60 seconds.

## Running a Benchmark

1. Set Mode to `BenchTune` in Server Settings
2. Press `Enter` to open BenchTune Setup
3. Select parameters to test with `Space`
4. Adjust parameter ranges (min/max/step)
5. Choose mode: `RuntimeOnly` or `Full` (toggle with `Alt+M`)
6. Edit prompt (`Alt+P`), n-predict (`Alt+N`), iterations (`Alt+I`)
7. Press `Enter` to start

The benchmark runs automatically. Progress is shown in the Active Model panel with a progress bar and current parameter display.

## Interpreting Results

Results include these metrics per test:

| Metric | Description | What it means |
|--------|-------------|---------------|
| **Prompt TPS** | Tokens processed per second during prompt evaluation | How fast the model reads your input |
| **Generation TPS** | Tokens generated per second | How fast the model produces output |
| **Combined TPS** | Total tokens (prompt + generation) per total time | Overall throughput |
| **Latency/Token** | Milliseconds per generated token | User-perceived responsiveness |
| **First Token Time** | Milliseconds until first token appears | Time to first response |

### Reading the Results Table

Results are displayed in a table sorted by combined TPS (descending). Use `n` (next) and `p` (previous) to navigate between results.

The **winner** section highlights the best configuration. Look for:
- Highest generation TPS for chat applications
- Lowest latency/token for interactive use
- Lowest first-token-time for responsive UX

### Impact Analysis

Each result includes an impact analysis showing how each parameter change affected performance:
- **Positive impact:** Parameter increased throughput
- **Negative impact:** Parameter decreased throughput
- **Neutral:** No significant change

## Exporting Results

Results can be exported in multiple formats. Press `e` in the results view to export:

| Format | Use Case |
|--------|----------|
| **Markdown table** | Documentation, sharing via chat/email |
| **JSON** | Programmatic analysis, CI/CD pipelines |
| **YAML** | Configuration files, version control |
| **HTML report** | Visual analysis with Chart.js charts |

The HTML report includes:
- Summary cards with key metrics
- Winner section with recommended configuration
- Impact analysis charts
- Per-parameter performance breakdowns
- Chart.js interactive charts for visual comparison

## Example Workflows

### Quick Chat Optimization

Goal: Find best settings for a chat application.

1. Set Mode to `BenchTune`
2. Enable: Temperature, Top-p, Top-k, Repeat Penalty
3. Select `RuntimeOnly` mode
4. Set iterations to 5 for stable results
5. Run benchmark
6. Export as Markdown for documentation

### Hardware Stress Test

Goal: Find maximum throughput on your hardware.

1. Set Mode to `BenchTune`
2. Enable: Threads, Batch Size, Context Length, Flash Attention
3. Select `Full` mode (tests server-level params)
4. Set iterations to 3
5. Run benchmark (may take 20-30 minutes)
6. Export as HTML for visual analysis

### Speculative Decoding Comparison

Goal: Compare speculative decoding methods.

1. Set Mode to `BenchTune`
2. Enable: Spec Type, Draft Tokens
3. Select `Full` mode
4. Set n-predict to 256 for meaningful speculative gains
5. Run benchmark
6. Compare first-token-time across spec types

## Tips and Best Practices

- **Use representative prompts:** Benchmark with prompts similar to your actual workload
- **Control variables:** Change one parameter at a time for clear attribution
- **Run multiple iterations:** Use at least 3 iterations to reduce variance
- **Warm up:** Let the model run for a few tokens before measuring (handled automatically)
- **Monitor system resources:** Watch for CPU/GPU saturation during benchmarks
- **Compare baselines:** Always benchmark against your current settings
- **Document results:** Export results to track improvements over time

## Troubleshooting

### Benchmark hangs or times out

- Increase `Test Timeout` in BenchTune Setup
- Reduce `n-predict` to shorter generation tasks
- Check server logs for errors

### Inconsistent results

- Increase `Iterations` for more stable averages
- Ensure no other processes are competing for GPU/CPU
- Close other applications using the GPU

### Low TPS values

- Verify GPU is being used (check VRAM usage)
- Try enabling Flash Attention if supported
- Increase batch size if VRAM allows
- Check that threads match your physical core count

### Benchmark fails on certain parameters

- Some parameters cannot be changed at runtime (use `Full` mode)
- Context length changes require server restart
- Backend-specific parameters may not be tunable
