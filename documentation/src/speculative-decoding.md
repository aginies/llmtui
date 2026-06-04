# Speculative Decoding

Speculative Decoding accelerates inference by using a smaller "draft" model to predict multiple tokens, which are then verified by the main model in parallel. This can significantly reduce generation time without sacrificing quality.

## How Speculative Decoding Works

```
Standard Decoding:
  [Token1] → [Token2] → [Token3] → [Token4] → [Token5]
  Time: 5 steps

Speculative Decoding:
  Draft:  [T1] → [T2] → [T3] → [T4] → [T5]
  Verify:  ✓    ✓    ✗    ✓    (rejects T4, respeculates)
  Time: 2 steps
```

The draft model generates tokens quickly. The main model verifies them in parallel. Accepted tokens are kept; rejected tokens trigger respeculation.

## Speculative Decoding Types

### Draft MTP (Multi-Token Prediction)

Uses a model's built-in draft tokens for speculative decoding. Requires a model with MTP architecture.

- **Best for:** Models specifically designed with MTP (e.g., Qwen2.5-MoE)
- **Performance:** Highest speedup when draft tokens are accurate
- **Auto-detection:** llm-manager automatically detects MTP models and enables this type

### draft-simple

Simple n-gram based speculative decoding.

- **Best for:** General use, no special model required
- **Performance:** Moderate speedup
- **Compatibility:** Works with any GGUF model

### draft-eagle3

EAGLE3 (Efficient Autoregressive Generation via Lookahead Decoding) speculative decoding.

- **Best for:** High-quality generation with good speedup
- **Performance:** Good balance of speed and quality
- **Requirements:** Model must support EAGLE3 architecture

### ngram-simple

N-gram based simple speculative decoding.

- **Best for:** Fast setup, minimal configuration
- **Performance:** Basic speedup
- **Compatibility:** Works with any model

### ngram-map-k

N-gram mapping with k-nearest neighbors.

- **Best for:** Models with repetitive patterns
- **Performance:** Variable, depends on text patterns
- **Complexity:** Higher memory usage

### ngram-mod

Modified n-gram speculative decoding.

- **Best for:** Experimental use
- **Performance:** Varies by model

### ngram-cache

N-gram cache-based speculative decoding.

- **Best for:** Repeated prompts or templates
- **Performance:** Excellent for templated generation

## Enabling Speculative Decoding

### In the TUI

1. Open LLM Settings (`F9`)
2. Navigate to **Speculative Decoding** section
3. Toggle **MTP** to enable
4. Select **Spec Type** from the dropdown
5. Set **Spec Draft N Max** (0-8, default: 0)

### In Config

```yaml
default:
  spec_type: "draft-mtp"
  draft_tokens: 4
```

### Auto-Detection

llm-manager automatically detects MTP models:

1. Load a model with MTP architecture
2. The app reads draft tokens from GGUF metadata
3. MTP is automatically enabled with appropriate settings
4. Draft token count is displayed in the Model Info panel

## Configuration

### Spec Type

Select the speculative decoding method:

| Spec Type | Model Requirement | Speedup | Quality |
|-----------|-------------------|---------|---------|
| `draft-mtp` | MTP architecture | High | Excellent |
| `draft-simple` | Any | Moderate | Good |
| `draft-eagle3` | EAGLE3 architecture | High | Excellent |
| `ngram-simple` | Any | Low-Moderate | Good |
| `ngram-map-k` | Any | Moderate | Good |
| `ngram-map-k4v` | Any | Moderate | Good |
| `ngram-mod` | Any | Variable | Good |
| `ngram-cache` | Any | High (templated) | Excellent |
| Off | N/A | None | N/A |

### Draft Tokens (N Max)

Maximum number of draft tokens per step:

| Value | Use Case | Tradeoff |
|-------|----------|----------|
| 0 | Disabled | No speedup, no overhead |
| 1-2 | Conservative | Low speedup, minimal rejection |
| 3-4 | Recommended | Good balance of speed and accuracy |
| 5-8 | Aggressive | Higher speedup, more rejections |
| 5-8 | Maximum | Highest potential speedup, high rejection rate |

**Optimal value depends on your model and text patterns.** Benchmark to find the best setting.

## Performance Expectations

### Typical Speedups

| Scenario | Expected Speedup |
|----------|------------------|
| MTP model with draft-mtp | 1.5-2.5× |
| General model with draft-simple | 1.2-1.5× |
| Templated text with ngram-cache | 2.0-3.0× |
| Creative writing | 1.1-1.3× |
| Code generation | 1.3-1.8× |

### Factors Affecting Performance

- **Draft accuracy:** Higher accuracy = more accepted tokens = better speedup
- **Model architecture:** Some models benefit more than others
- **Text patterns:** Repetitive patterns are easier to speculate
- **Context length:** Longer contexts may reduce speculation accuracy
- **Draft token count:** Too many drafts increase rejection rate

## Benchmarking Speculative Decoding

Use Benchmark Tuning to find optimal speculative decoding settings:

1. Set Mode to `BenchTune`
2. Enable `Spec Type` and `Draft Tokens`
3. Run benchmark with different spec types
4. Compare generation TPS and latency
5. Export results to find the best configuration

## Troubleshooting

### No Speedup Observed

- Check draft accuracy (too many rejections)
- Reduce `Draft N Max` to lower rejection rate
- Try a different spec type
- Verify model supports speculative decoding

### Quality Degradation

- Reduce `Draft N Max`
- Switch to a more accurate spec type (e.g., draft-mtp)
- Increase temperature slightly to compensate
- Check draft token count matches model capabilities

### Model Not Detected as MTP

- Verify model has MTP architecture (check GGUF metadata)
- Ensure draft tokens are present in metadata
- Check llama.cpp version supports MTP
- Review server logs for MTP detection messages

### High Rejection Rate

- Reduce `Draft N Max`
- Try a different spec type
- Check if model is suitable for speculative decoding
- Verify draft model matches main model architecture

## Best Practices

### Choosing a Spec Type

- **MTP models:** Always use `draft-mtp`
- **General purpose:** Start with `draft-simple` or `draft-eagle3`
- **Templated generation:** Use `ngram-cache`
- **Code generation:** Use `draft-mtp` or `draft-simple`
- **Creative writing:** Use `draft-simple`

### Setting Draft Tokens

- Start with 4 as a baseline
- Increase if acceptance rate is high (>70%)
- Decrease if rejection rate is high (<50%)
- Monitor first-token-time for interactive use

### Monitoring Performance

Track these metrics:
- **Acceptance rate:** Percentage of draft tokens accepted
- **Generation TPS:** Tokens per second with speculation
- **First-token-time:** Time until first token appears
- **Latency:** Milliseconds per token

### When to Disable

Disable speculative decoding when:
- Generating very short responses (<32 tokens)
- Using models without draft token support
- Experiencing quality degradation
- Running on CPU-only systems with limited resources

## Advanced Usage

### Combining with Other Optimizations

Speculative decoding works well with:
- **Flash Attention:** Reduces memory usage, improves speed
- **KV Cache Quantization:** Frees VRAM for larger contexts
- **Router Mode:** Compare speculative vs non-speculative models *(Work In Progress)*

### Dynamic Adjustment

Adjust speculative decoding settings based on workload:
- **Interactive chat:** Lower draft tokens (2-4) for responsiveness
- **Batch processing:** Higher draft tokens (6-8) for throughput
- **Creative generation:** Moderate draft tokens (4-6) for quality

### Custom Draft Models

For advanced users, custom draft models can be trained:
1. Collect generation data from your domain
2. Train a small draft model on the data
3. Use the draft model for speculation
4. Monitor and adjust based on acceptance rates
