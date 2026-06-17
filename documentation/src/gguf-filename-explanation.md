# GGUF Filename Explanation

GGUF filenames encode the model's architecture, quantization, and source. Press `Ctrl+G` from any panel to open a popup that parses and explains the filename.

![Model Info Panel](images/info_model.png)

## How It Works

The parser splits the filename into segments and provides a description for each:

- **Model family** — e.g., "Qwen3.6-35B-A3B", "Llama-3.1-8B"
- **Unsloth Dynamic** — Indicates the model was fine-tuned using Unsloth's dynamic methodology
- **Quantization** — e.g., "Q4_K_M", "Q5_K_S"
- **Extension** — ".gguf"

## Quantization Legend

| Quant | Description |
|-------|-------------|
| Q4_0 | 4-bit quantization (legacy) |
| Q4_K_M | 4-bit mixed quantization (recommended) |
| Q5_K_M | 5-bit mixed quantization |
| Q5_K_S | 5-bit small quantization |
| Q8_0 | 8-bit quantization |
| F16 | Floating-point 16-bit (unquantized) |

## Model Families

The parser recognizes common model families and provides specific explanations:

- **Qwen** — Alibaba's Qwen models (dense and MoE)
- **Llama** — Meta's Llama models
- **Gemma** — Google's Gemma models
- **Mistral** — Mistral AI models
- **Phi** — Microsoft's Phi models
- **And more** — Custom explanations for other architectures

## Auto-Detection of MoE Models

For MoE (Mixture-of-Experts) models, the parser extracts and displays the expert count and parameter breakdown. For example, Qwen3.6-35B-A3B indicates a 35B total parameter model with 3.5B active parameters per token.

## Unsloth Dynamic

When a model has been fine-tuned using Unsloth's dynamic methodology, the filename includes "Unsloth Dynamic". This indicates the model was optimized with Unsloth's dynamic quantization approach, which typically yields better quality at the same quantization level.
