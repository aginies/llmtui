# Getting Started

## Installation

### From source

```bash
git clone https://github.com/aginies/llmtui.git
cd llmtui
cargo build --release
```

### Using the build script

A convenience script is included for common operations:

```bash
./build.sh build      # Build (debug)
./build.sh run        # Build and run
./build.sh release    # Release build
./build.sh clean      # Remove build artifacts
./build.sh format     # Format code
./build.sh clippy     # Run clippy
```

## First Run

On first launch, llm-manager creates a default configuration in `~/.config/llm-manager/config.yaml` and sets up the models directory at `~/.local/share/llm-manager/models/`.

```bash
cargo run
```

The application will:

1. Load (or create) the config file
2. Discover any `.gguf` files in the models directory
3. Start the TUI

## Navigating the Interface

The TUI is divided into several panels:

- **Models panel** (left) — list of local GGUF models
- **Settings panel** (right) — server and LLM settings
- **Log panel** (bottom) — live output from llama.cpp
- **Download panel** — appears when downloading files

Use `Tab` to cycle between panels, and `Ctrl+H` for panel-specific help.

## Searching for Models

To search HuggingFace for models:

1. Press `/` to enter search mode
2. Type your query and press `Enter`
3. Results appear sorted by relevance by default
4. Press `S` to cycle sort order (Relevance / Downloads / Likes / Trending / Created)
5. Press `B` to go back one page, or scroll down at the bottom for more results
6. Press `R` to fetch and view the model's README

### Downloading Models

To download a model from HuggingFace:

1. Press `/` to enter search mode
2. Type your query and press `Enter`
3. Press `l` on a result to browse available GGUF files
4. Select a file and press `Enter` to download
5. Press `c` to cancel the download at any time

The download progress is shown in the Download panel. Once complete, the model appears in the Models panel (in your models directory).

### Loading Models

Once a model is downloaded (or has one locally in your models directory):

1. Select the model in the Models panel
2. Press `l` (or `Enter`) to load it

The loading process shows a progress bar with phases:

- Server starting
- Loading model weights
- Loading metadata
- Loading tensors (with GPU layer count)
- Server listening
- Ready

## Using Serve Mode

You can also start a model directly from the command line:

```bash
./build.sh serve --model /path/to/model.gguf
```

Or with a settings profile:

```bash
./build.sh serve --model model.gguf --profile qwen
```

### API Proxy

Start with an OpenAI-compatible API proxy:

```bash
./build.sh serve --model model.gguf --api-port 49222
```

With authentication:

```bash
./build.sh serve --model model.gguf --api-port 49222 --api-key secret
```

The API proxy forwards requests to the llama-server instance and supports all llama.cpp endpoints including chat completions, embeddings, and more.
