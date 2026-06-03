# Distributed Inference (RPC Workers)

RPC Workers enable distributed inference across multiple machines. Each worker runs a `llama-rpc-server` that exposes part of a model or a complete model for remote inference.

## What is Distributed Inference?

Distributed inference splits model computation across multiple machines, allowing you to:
- Combine GPU resources from multiple machines
- Run models larger than a single GPU's VRAM
- Reduce latency by placing workers closer to clients
- Scale inference horizontally

## Architecture

```
                    ┌─────────────────┐
                    │   llm-manager   │
                    │   (TUI Client)  │
                    └────────┬────────┘
                             │ RPC
              ┌──────────────┼──────────────┐
              │              │              │
      ┌───────▼──────┐ ┌────▼─────┐ ┌──────▼──────┐
      │  Worker 1    │ │ Worker 2 │ │  Worker 3   │
      │  (GPU: A100) │ │(GPU: 4090)│ │ (GPU: A6000)│
      └──────────────┘ └──────────┘ └─────────────┘
```

## Setting Up Workers

### Prerequisites

- `llama-rpc-server` binary on each worker machine
- Network connectivity between client and workers
- Consistent llama.cpp versions across all machines

### Installing llama-rpc-server

Each worker machine needs the RPC server binary:

```bash
# Download from llama.cpp releases
wget https://github.com/ggml-org/llama.cpp/releases/download/b4100/llama-rpc-server-ubuntu-x64.tar.gz
tar -xzf llama-rpc-server-ubuntu-x64.tar.gz
```

### Starting a Worker

Start the RPC server on each worker machine:

```bash
./llama-rpc-server \
  --model /path/to/model.gguf \
  --host 0.0.0.0 \
  --port 50052
```

Parameters:
- `--model`: Path to the GGUF model file
- `--host`: Bind address (use `0.0.0.0` for network access)
- `--port`: RPC port (default: 50052)

## Managing Workers in TUI

### Opening the RPC Manager

1. Open Server Settings (`F2`)
2. Navigate to **RPC Workers**
3. Press `Enter` to open the RPC Workers manager

### Adding a Worker

1. In the RPC Workers manager, press `n` to add a new worker
2. Enter worker details: `[Name], [IP], [Port]`
   - Example: `GPU-A100, 192.168.1.10, 50052`
   - Or: `192.168.1.10, 50052` (name auto-generated)
   - Or: `192.168.1.10` (name and port use defaults)
3. Press `Enter` to save

### Editing a Worker

1. Select the worker in the list
2. Press `e` to edit
3. Modify the details
4. Press `Enter` to save

### Deleting a Worker

1. Select the worker
2. Press `d` to delete
3. Confirm deletion

### Selecting Workers

1. Use `Space` to toggle worker selection
2. Selected workers are combined into the `--rpc` flag when starting the server
3. Only selected workers are used for inference

## Worker Configuration

### Worker Properties

Each worker has these properties:

| Property | Description | Example |
|----------|-------------|---------|
| **Name** | Human-readable identifier | `GPU-A100-Rack1` |
| **IP** | Network address of the worker | `192.168.1.10` |
| **Port** | RPC server port (default: 50052) | `50052` |
| **Selected** | Whether to use this worker | `true`/`false` |

### Network Configuration

#### Firewall Rules

Ensure port 50052 (or your chosen port) is open:

```bash
# Ubuntu/Debian
sudo ufw allow 50052/tcp

# CentOS/RHEL
sudo firewall-cmd --add-port=50052/tcp --permanent
sudo firewall-cmd --reload
```

#### SSH Tunneling

For secure connections without opening ports:

```bash
ssh -L 50052:localhost:50052 user@remote-worker
```

Then use `localhost:50052` as the worker address in llm-manager.

## Using Distributed Inference

### Loading a Model Across Workers

1. Configure and select your workers
2. Set Mode to `Router` or `Normal`
3. Load your model
4. The model is distributed across selected workers

### Monitoring Workers

The Active Model panel shows total VRAM usage across all workers. Per-worker metrics are not yet displayed in the TUI.

### RPC Settings

Configure RPC behavior in LLM Settings:

| Setting | Description |
|---------|-------------|
| **RPC** | Comma-separated list of worker endpoints |
| **Tensor Split** | Fraction of model per GPU (for multi-GPU workers) |

Example RPC string: `localhost:50052,192.168.1.10:50052`

## Use Cases

### Multi-GPU Workstation

Combine multiple GPUs in a single workstation:

```
Worker 1: localhost:50052 (GPU 0 - RTX 4090)
Worker 2: localhost:50053 (GPU 1 - RTX 4090)
```

### Distributed Data Center

Spread inference across a data center:

```
Worker 1: 10.0.1.10:50052 (A100 80GB)
Worker 2: 10.0.1.11:50052 (A100 80GB)
Worker 3: 10.0.1.12:50052 (A100 80GB)
```

### Edge Computing

Deploy workers close to end users:

```
Worker 1: us-east.worker.example.com:50052
Worker 2: eu-west.worker.example.com:50052
Worker 3: ap-southeast.worker.example.com:50052
```

## Troubleshooting

### Worker Connection Failed

- Verify worker IP address is correct
- Check firewall rules allow port 50052
- Ensure `llama-rpc-server` is running on the worker
- Test connectivity: `nc -zv <worker-ip> 50052`

### Model Fails to Load

- Verify all workers have sufficient VRAM
- Check network latency between workers (should be <1ms for optimal performance)
- Ensure llama.cpp versions match across all workers
- Review worker logs for errors

### Slow Inference

- Check network bandwidth between client and workers
- Verify workers are not CPU-bound
- Monitor GPU utilization on each worker
- Consider reducing model size or context length

### Worker Disconnected

- Check worker machine is still running
- Verify network connectivity
- Restart `llama-rpc-server` if needed
- Re-select the worker in llm-manager

## Best Practices

### Hardware Selection

- Use GPUs with similar capabilities for balanced performance
- Prefer GPUs with high memory bandwidth (A100, H100, RTX 4090)
- Ensure sufficient VRAM for your model size

### Network Setup

- Use wired connections (Ethernet) for workers
- Minimize network latency between workers (<1ms ideal)
- Use dedicated network interfaces for RPC traffic if possible

### Monitoring

- Monitor VRAM usage on each worker
- Track network latency and throughput
- Set up alerts for worker disconnections
- Log inference metrics for capacity planning

### Security

- Use SSH tunneling for untrusted networks
- Implement firewall rules to restrict access
- Consider TLS for RPC communication (if supported by your llama.cpp version)
- Use strong authentication for remote workers

## Advanced Configuration

### Custom Ports

Each worker can use a different port:

```
Worker 1: 192.168.1.10:50052
Worker 2: 192.168.1.11:50053
Worker 3: 192.168.1.12:50054
```

### Dynamic Worker Management

Workers can be added/removed without restarting the server:

1. Open RPC Workers manager
2. Add new worker
3. Select the worker
4. Reload the model to incorporate the new worker

### Worker Health Checks

The app automatically checks worker health:
- Connection status shown in RPC Workers manager
- Failed workers are marked and excluded from inference
- Reconnection attempts are made automatically
