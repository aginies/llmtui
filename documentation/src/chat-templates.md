# Chat Templates

Chat templates define how the model formats conversations for chat completion. llm-manager supports three modes:

## Auto (Detect from GGUF)

When set to **Auto**, the app reads the model's GGUF architecture metadata and automatically selects the correct llama.cpp built-in chat template. This is the recommended mode for most use cases — it works out of the box with any model.

## Built-in Template Names

You can also select specific llama.cpp built-in templates by name. The available templates depend on the model and are auto-detected from the GGUF metadata. These are the same templates llama.cpp uses internally.

## Browse Directory

Select **Browse directory** to pick a custom `.jinja` chat template file from your filesystem. The app searches for `.jinja` files recursively in:

- `<app directory>/locales/chat_templates/` (for serve mode)
- `~/.config/llm-manager/chat_templates/` (for TUI mode)

You can also configure a custom directory by setting the `chat_templates_dir` in your config.

## None

Select **None** to disable any chat template. The model will receive raw inputs without any conversation formatting. Useful for non-chat tasks like completion or embedding.

## Chat Template Kwargs

Chat template kwargs allow you to inject additional parameters into the chat template. These are passed as a JSON string to llama.cpp's `--chat-template-kwargs` flag.

For example, some models support an `enable_thinking` parameter that controls whether the model outputs its reasoning:

```json
{"enable_thinking": false}
```

Open the chat template kwargs editor by pressing `Alt+C` in the LLM Settings panel.

## Jinja Template Files

Custom `.jinja` files use the Jinja2 templating syntax. They are loaded and applied at inference time. Example structure:

```jinja2
<|im_start|>system
{{ system_prompt }}<|im_end|>
<|im_start|>user
{{ prompt }}<|im_end|>
<|im_start|>assistant
```

Place custom templates in the chat_templates directory (see Browse Directory above).

## Configuration

Chat template settings are stored per-model in the per-model YAML config or in the LLM Settings panel:

| Config Key | Type | Description |
|-----------|------|-------------|
| `jinja` | bool | Enable Jinja chat template (true by default) |
| `chat_template` | string/null | Custom chat template name or file path |
| `auto_chat_template` | bool | Auto-detect template from GGUF metadata |
| `chat_template_kwargs` | string/null | JSON string for chat template parameters |
