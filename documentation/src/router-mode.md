# Router Mode & Multi-Model Inference

> **⚠️ Work In Progress:** Router Mode is currently under active development. It is not yet selectable through the TUI interface — the mode switcher cycles `Normal → Bench → BenchTune → Normal` only. Router mode can be enabled manually by setting `server_mode: router` in `~/.config/llm-manager/config.yaml`. The backend infrastructure (server spawning, `/models/load`, `/models/unload`, `/models` endpoints) is implemented and functional.

Router Mode enables loading and managing multiple models simultaneously through a single llama-server instance. This is useful for A/B testing models, building model routing systems, or comparing different models in a shared environment.

## What is Router Mode?

Router Mode uses llama.cpp's router API to load multiple models into a single server process. Each model is addressed by its unique identifier, allowing clients to route requests to specific models.

## Enabling Router Mode

1. Open Server Settings (`F2` or navigate to Server Settings panel)
2. Set **Mode** to `Router`
3. Set **Router Max Models** to your desired limit (default: 4)
4. Load your first model normally

## Loading Models

In Router Mode, models are loaded via the API endpoint `/models/load`:

```bash
curl -X POST http://localhost:8080/models/load \
  -H "Content-Type: application/json" \
  -d '{"model": "model_name"}'
```

Or through the TUI:
1. Select a model in the Models panel
2. Press `l` or `Enter` to load it

Each loaded model shows its status in the Models panel with the port and PID.

## Managing Models

### Listing Loaded Models

Get all loaded models and their status:

```bash
curl http://localhost:8080/models
```

Response format:
```json
[
  {
    "id": "model_name",
    "object": "model",
    "owned_by": "user",
    "path": "/path/to/model.gguf"
  }
]
```

### Unloading Models

Unload a specific model:

```bash
curl -X POST http://localhost:8080/models/unload \
  -H "Content-Type: application/json" \
  -d '{"model": "model_name"}'
```

Or in the TUI:
1. Select a loaded model
2. Press `u` to unload it

### Deleting Models

Delete a model (moves to unused directory):
1. Select the model
2. Press `Ctrl+D`
3. Confirm deletion

## VRAM Management

### Understanding VRAM Usage

Each loaded model consumes VRAM proportional to:
- Model size (quantization level)
- GPU layers offloaded
- Context length
- Batch size

### Monitoring VRAM

The Active Model panel shows:
- **VRAM:** GPU memory used/total per model
- **Total VRAM:** Sum of all model VRAM usage
- **Context usage:** Progress bar showing ctx_used/ctx_max

### VRAM Estimation

The app computes VRAM estimates based on:
- Model file size (with MoE expert ratio applied to FFN portion for mixture-of-experts models)
- GPU layers mode (Auto/Specific/All)
- KV cache settings (Flash Attention, quantization, YaRN RoPE scale)
- Activation overhead (8× multiplier)
- Fixed overhead (3.8% of max VRAM)

The estimate is shown in the LLM Settings title (e.g., "VRAM ~= 8.2 GB").

### Best Practices

- **Leave headroom:** Keep 10-20% VRAM free for KV cache and activations
- **Use lower quantization:** Q4_K_M or Q5_K_M for better multi-model support
- **Reduce context length:** Shorter contexts use less VRAM
- **Monitor Total VRAM:** The Active Model panel shows combined usage

## Configuration

### Router Max Models

Set the maximum number of models that can be loaded simultaneously.

In config.yaml:
```yaml
default:
  router_max_models: 4
```

In the TUI:
1. Open Server Settings
2. Navigate to Router Max Models
3. Adjust value (1-10)

### Per-Model Settings

Each model can have its own settings:
- Context length
- GPU layers
- Temperature
- Sampling parameters

Settings are stored in `~/.config/llm-manager/models/<name>.yaml`.

## API Usage

### Chat Completions

Route to a specific model:

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "model_name",
    "messages": [{"role": "user", "content": "Hello"}]
  }'
```

### Streaming

Enable SSE streaming:

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "model_name",
    "messages": [{"role": "user", "content": "Hello"}],
    "stream": true
  }'
```

## Use Cases

### Model Comparison

Load multiple models to compare outputs:

1. Load Model A and Model B
2. Send the same prompt to both
3. Compare generation quality and speed

### Specialized Models

Run different models for different tasks:

- **Coder model:** Optimized for code generation
- **Math model:** Optimized for mathematical reasoning
- **General model:** For everyday conversations

Route requests based on the task type.

### A/B Testing

Test model updates without downtime:

1. Load the old model
2. Load the new model
3. Route some traffic to each
4. Compare metrics

## Limitations

### VRAM Constraints

- Each model consumes VRAM independently
- Total VRAM cannot exceed GPU capacity
- KV cache for each model is allocated separately

### Model Conflicts

- Models with different architectures may have compatibility issues
- Some parameters (like context length) are model-specific
- Loading/unloading affects all models in the server

### Performance

- Shared server means shared CPU/GPU resources
- Heavy models may impact lighter model performance
- Monitor system resources during multi-model operation

## Troubleshooting

### "VRAM exceeded" error

- Reduce GPU layers for loaded models
- Use lower quantization models
- Unload unused models
- Reduce context length

### Model fails to load

- Check model path is correct
- Verify model file exists and is readable
- Check available VRAM
- Review server logs for specific errors

### Slow performance

- Reduce number of loaded models
- Check for resource contention
- Verify GPU is being used (not CPU fallback)
- Monitor system resources

### Router API not responding

- Verify server is running on the expected port
- Check network connectivity
- Ensure router mode is enabled
- Review server logs for errors
