export const WS_METRICS = [
    { key: 'model_name', label: 'Model', type: 'text' },
    { key: 'tps', label: 'TPS', type: 'number', unit: 't/s' },
    { key: 'prompt_tps', label: 'Prompt TPS', type: 'number', unit: 't/s' },
    { key: 'gen_tps', label: 'Gen TPS', type: 'number', unit: 't/s' },
    { key: 'ctx', label: 'Ctx', type: 'ratio', used: 'ctx_used', max: 'ctx_max', unit: 'tokens' },
    { key: 'vram', label: 'VRAM', type: 'ratio_gb', used: 'gpu_mem_used', total: 'gpu_mem_total' },
    { key: 'ram', label: 'RAM', type: 'gb', field: 'ram_used' },
    { key: 'cpu', label: 'CPU', type: 'percent', field: 'cpu_usage' },
    { key: 'decoded_tokens', label: 'Decoded', type: 'number' },
    { key: 'prompt_tokens', label: 'Prompt Eval', type: 'number', unit: 'tokens' },
    { key: 'prompt_progress', label: 'Prompt Progress', type: 'ratio_pct', used: 'prompt_progress', max: 1.0 },
];

export const METRIC_GROUPS = [
    {
        name: 'Model',
        metrics: ['model_name'],
    },
    {
        name: 'Performance',
        metrics: ['tps', 'prompt_tps', 'gen_tps', 'decoded_tokens', 'prompt_tokens', 'prompt_progress'],
    },
    {
        name: 'Resources',
        metrics: ['ctx', 'vram', 'ram', 'cpu'],
    },
];
