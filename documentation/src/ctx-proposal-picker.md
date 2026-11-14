# Ctx / KV-cache / MTP Proposal Picker (F8)

Pressing **F8** with a model selected (in the **Models** or **LLM Settings** panel) opens the **proposal picker**: a list of feasible **context length / KV-cache type / MTP** combinations that fit in the detected GPU VRAM, ranked by quality.

```
┌─ CTX / KV Cache Proposals — qwen3-30b-a3b.gguf ──────────────────────────┐
│ [↑↓] Select   [↵] Apply   [⎋] Cancel                                     │
│ > ★ ctx 100 663 │ K off │ V off │ VRAM 21.4/24 GiB │ Best quality        │
│   ctx 131 072 │ K q8_0 │ V q8_0 │ VRAM 22.9/24 GiB │ Best CTX            │
│   ctx 100 663 │ K q5_0 │ V q5_0 │ MTP 6 │ VRAM 23.5/24 GiB │ Best perf   │
│ ...                                                                      │
│ ★ = recommended (first to reach 100k ctx) · only proposals that fit      │
│   VRAM are listed · best quality first                                   │
└──────────────────────────────────────────────────────────────────────────┘
```

## Keys

| Key | Action |
|-----|--------|
| `↑` / `↓` / `j` / `k` | Navigate proposals |
| `Home` / `End` | Jump to first / last |
| `Enter` | Apply the selected proposal |
| `Esc` / `q` | Cancel |

## Requirements

- A model is selected (Models or LLM Settings panel)
- The model's GGUF metadata is available (parsed and cached)
- GPU VRAM is detected (live server metrics first, hardware detection as fallback)
- GPU offload is not disabled — a `GPU Layers = 0` (CPU-only) setting produces no proposals

If any of these is missing, a warning is logged (e.g. `No GPU VRAM detected — cannot compute ctx proposals`) and the picker does not open.

## How Proposals Are Generated

1. **VRAM budget** — 97% of the detected total VRAM, leaving headroom for the driver and fragmentation instead of sitting exactly at the card's capacity.
2. **KV quality ladder** — candidate (K, V) cache-type pairs, best quality first:

   | # | K | V |
   |---|---|---|
   | 1 | off (server default, f16) | off |
   | 2 | q8_0 | q8_0 |
   | 3 | q8_0 | q5_1 |
   | 4 | q5_1 | q5_1 |
   | 5 | q5_1 | q5_0 |
   | 6 | q5_0 | q5_0 |
   | 7 | q5_0 | q4_1 |
   | 8 | q4_1 | q4_1 |
   | 9 | q4_0 | q4_0 |
   | 10 | iq4_nl | q4_0 |

   **Model-quantization floor:** rungs below the model's own weight quantization are never proposed — e.g. a Q8_0 model is never offered a q4_0 cache. The first rung is always *off* (llama.cpp's f16 default); quantized caches only appear as fallbacks for when the model doesn't fit otherwise.
3. **MTP variants** — for models with MTP (nextn) layers, KV tiers are expanded over MTP draft levels: 0 (off), 1, 2, the midpoint, and the cap — 6 draft tokens normally, 3 for "big" models (≥ 27B active parameters), since each draft token eats context room. Models without MTP layers get no MTP variants at all.
4. **Solve** — for each combination, the maximum context that fits in the budget is solved with the same VRAM model as the LLM Settings estimate, rounded down to 1k-token steps. Combinations whose weights + minimum KV cache already exceed VRAM are skipped entirely — **only feasible proposals are listed**.
5. **Big-model filter** — for models with ≥ 27B active parameters, proposals below 64k context are dropped as unusable in practice.
6. **Cap & de-duplication** — at most 15 rows; identical outcomes (same K/V/MTP/ctx) are de-duplicated.

## Recommendation (★)

The ★ marks the first (best-quality) proposal that reaches the **100k ctx target** — clamped to the model's native training context (`n_ctx_train`) when that is smaller. The picker opens with the recommended row preselected.

## Trade-off Notes

Each row is labelled with the trade-off it represents relative to the other proposals:

| Note | Meaning |
|------|---------|
| **Best quality** | Highest KV-cache precision (best output fidelity) |
| **Best CTX** | Largest context window |
| **Best performance** | Fastest generation (light KV cache + max MTP draft) |
| **Balanced quality/CTX** | Good fidelity with a large context |
| **Balanced quality/perf** | Good fidelity with fast generation |
| **Balanced CTX/perf** | Large context with fast generation |
| **Balanced** | Middle-of-the-road compromise |

## Applying a Proposal

`Enter` writes the proposal into the model's LLM settings:

- `context_length` = the proposal's ctx
- `cache_type_k` / `cache_type_v` = the proposal's K/V types (*off* = server default)
- For MTP variants: `spec_type = draft-mtp` and `draft_tokens` = the proposal's MTP level

Settings are marked dirty (the VRAM estimate is recomputed) and persisted to the per-model config. The log records e.g. `Applied proposal: ctx 100 663 · K q8_0 · V q8_0`.
