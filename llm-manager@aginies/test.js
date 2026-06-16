const fs = require('node:fs');
const assert = require('node:assert');

// Test buildWsUrl function
function buildWsUrl(metricsUrl, authEnabled) {
    try {
        const match = metricsUrl.match(/^(https?:)\/\/([^\/?#]+)([^?#]*)(?:\?([^#]*))?/);
        if (!match) {
            throw new Error('Invalid URL format');
        }
        const protocol = match[1];
        const host = match[2];
        const query = match[4] || '';

        const wsProtocol = protocol === 'https:' ? 'wss:' : 'ws:';
        let auth = null;
        if (authEnabled && query) {
            const params = query.split('&');
            for (const param of params) {
                const [key, value] = param.split('=');
                if (key === 'auth') {
                    auth = decodeURIComponent(value);
                    break;
                }
            }
        }
        return {
            wsUrl: `${wsProtocol}//${host}/ws${auth ? '?auth=' + encodeURIComponent(auth) : ''}`,
            hasAuth: !!auth,
        };
    } catch (e) {
        return { wsUrl: 'ws://127.0.0.1:8080/ws', hasAuth: false };
    }
}

console.log('Testing buildWsUrl...');

// Test 1: Basic HTTP URL
const result1 = buildWsUrl('http://127.0.0.1:8080/metrics', true);
assert.strictEqual(result1.wsUrl, 'ws://127.0.0.1:8080/ws');
assert.strictEqual(result1.hasAuth, false);
console.log('✓ Test 1 passed: Basic HTTP URL');

// Test 2: HTTP URL with auth
const result2 = buildWsUrl('http://127.0.0.1:8080/metrics?auth=secret123', true);
assert.strictEqual(result2.wsUrl, 'ws://127.0.0.1:8080/ws?auth=secret123');
assert.strictEqual(result2.hasAuth, true);
console.log('✓ Test 2 passed: HTTP URL with auth');

// Test 3: HTTPS URL
const result3 = buildWsUrl('https://example.com/metrics', true);
assert.strictEqual(result3.wsUrl, 'wss://example.com/ws');
assert.strictEqual(result3.hasAuth, false);
console.log('✓ Test 3 passed: HTTPS URL');

// Test 4: HTTPS URL with auth
const result4 = buildWsUrl('https://example.com/metrics?auth=token456', true);
assert.strictEqual(result4.wsUrl, 'wss://example.com/ws?auth=token456');
assert.strictEqual(result4.hasAuth, true);
console.log('✓ Test 4 passed: HTTPS URL with auth');

// Test 5: Auth disabled
const result5 = buildWsUrl('http://127.0.0.1:8080/metrics?auth=secret123', false);
assert.strictEqual(result5.wsUrl, 'ws://127.0.0.1:8080/ws');
assert.strictEqual(result5.hasAuth, false);
console.log('✓ Test 5 passed: Auth disabled ignores query param');

// Test 6: Invalid URL
const result6 = buildWsUrl('not-a-url', true);
assert.strictEqual(result6.wsUrl, 'ws://127.0.0.1:8080/ws');
assert.strictEqual(result6.hasAuth, false);
console.log('✓ Test 6 passed: Invalid URL fallback');

// Test 7: Auth with special characters
const result7 = buildWsUrl('http://127.0.0.1:8080/metrics?auth=abc%20def', true);
assert.strictEqual(result7.wsUrl, 'ws://127.0.0.1:8080/ws?auth=abc%20def');
assert.strictEqual(result7.hasAuth, true);
console.log('✓ Test 7 passed: Auth with special characters preserved');

// Test formatNumber function
function formatNumber(value, decimals) {
    if (value === undefined || value === null || isNaN(value)) return 'N/A';
    return value.toFixed(decimals);
}

console.log('\nTesting formatNumber...');

assert.strictEqual(formatNumber(12.3456, 2), '12.35');
assert.strictEqual(formatNumber(12.3456, 0), '12');
assert.strictEqual(formatNumber(12.3456, 1), '12.3');
assert.strictEqual(formatNumber(undefined, 2), 'N/A');
assert.strictEqual(formatNumber(null, 2), 'N/A');
assert.strictEqual(formatNumber(NaN, 2), 'N/A');
console.log('✓ formatNumber tests passed');

// Test formatGB function
function formatGB(bytes) {
    if (bytes === undefined || bytes === null || isNaN(bytes)) return 'N/A';
    return (bytes / 1024 / 1024 / 1024).toFixed(1) + ' GB';
}

console.log('\nTesting formatGB...');

assert.strictEqual(formatGB(1073741824), '1.0 GB');
assert.strictEqual(formatGB(2147483648), '2.0 GB');
assert.strictEqual(formatGB(0), '0.0 GB');
assert.strictEqual(formatGB(undefined), 'N/A');
assert.strictEqual(formatGB(null), 'N/A');
console.log('✓ formatGB tests passed');

// Test formatBytes function
function formatBytes(bytes) {
    if (bytes === undefined || bytes === null || isNaN(bytes)) return 'N/A';
    if (bytes >= 1e9) return (bytes / 1e9).toFixed(1) + ' GB';
    if (bytes >= 1e6) return (bytes / 1e6).toFixed(1) + ' MB';
    return Math.round(bytes) + ' B';
}

console.log('\nTesting formatBytes...');

assert.strictEqual(formatBytes(1073741824), '1.1 GB');
assert.strictEqual(formatBytes(5242880), '5.2 MB');
assert.strictEqual(formatBytes(1024), '1024 B');
assert.strictEqual(formatBytes(undefined), 'N/A');
console.log('✓ formatBytes tests passed');

// Test getVramColor function
function getVramColor(percent) {
    if (percent > 80) return 'llm-value-bad';
    if (percent > 50) return 'llm-value-warn';
    return 'llm-value-good';
}

console.log('\nTesting getVramColor...');

assert.strictEqual(getVramColor(0), 'llm-value-good');
assert.strictEqual(getVramColor(50), 'llm-value-good');
assert.strictEqual(getVramColor(51), 'llm-value-warn');
assert.strictEqual(getVramColor(75), 'llm-value-warn');
assert.strictEqual(getVramColor(80), 'llm-value-warn');
assert.strictEqual(getVramColor(81), 'llm-value-bad');
assert.strictEqual(getVramColor(100), 'llm-value-bad');
console.log('✓ getVramColor tests passed');

// Test truncateModelName function
function truncateModelName(name, maxLen) {
    if (!name) return 'No model';
    if (name.length <= maxLen) return name;
    const suffix = name.endsWith('.gguf') ? '.gguf' : '';
    const ext = suffix.length;
    const available = maxLen - ext;
    if (available <= 0) return name.substring(0, maxLen);
    return name.substring(0, available) + '...' + suffix;
}

console.log('\nTesting truncateModelName...');

assert.strictEqual(truncateModelName('my-model.gguf', 20), 'my-model.gguf');
assert.strictEqual(truncateModelName('my-model.gguf', 10), 'my-mo....gguf');
assert.strictEqual(truncateModelName('a', 5), 'a');
assert.strictEqual(truncateModelName(undefined, 10), 'No model');
assert.strictEqual(truncateModelName('', 10), 'No model');
console.log('✓ truncateModelName tests passed');

// Test metric value formatting
const WS_METRICS = [
    { key: 'model_name', label: 'Model', type: 'text' },
    { key: 'state', label: 'State', type: 'badge' },
    { key: 'tps', label: 'TPS', type: 'number', unit: 't/s' },
    { key: 'prompt_tps', label: 'Prompt TPS', type: 'number', unit: 't/s' },
    { key: 'gen_tps', label: 'Gen TPS', type: 'number', unit: 't/s' },
    { key: 'ctx', label: 'Ctx', type: 'ratio', used: 'ctx_used', max: 'ctx_max', unit: 'tokens' },
    { key: 'vram', label: 'VRAM', type: 'ratio_gb', used: 'gpu_mem_used', total: 'gpu_mem_total' },
    { key: 'ram', label: 'RAM', type: 'gb', key: 'ram_used' },
    { key: 'cpu', label: 'CPU', type: 'percent', key: 'cpu_usage' },
    { key: 'decoded_tokens', label: 'Decoded', type: 'number' },
];

console.log('\nTesting WS_METRICS definition...');

assert.strictEqual(WS_METRICS.length, 10);
assert.strictEqual(WS_METRICS[0].key, 'model_name');
assert.strictEqual(WS_METRICS[0].type, 'text');
assert.strictEqual(WS_METRICS[5].type, 'ratio');
assert.strictEqual(WS_METRICS[5].used, 'ctx_used');
assert.strictEqual(WS_METRICS[6].type, 'ratio_gb');
assert.strictEqual(WS_METRICS[8].type, 'percent');
console.log('✓ WS_METRICS definition valid');

// Test WebSocket message parsing
function parseWsMetrics(jsonStr) {
    try {
        return JSON.parse(jsonStr);
    } catch (e) {
        return null;
    }
}

console.log('\nTesting WebSocket message parsing...');

const mockMetrics = JSON.stringify({
    model_name: 'llama3.gguf',
    loaded: true,
    state: 'loaded',
    tps: 42.5,
    prompt_tps: 1234.5,
    gen_tps: 42.3,
    ctx_used: 2048,
    ctx_max: 8192,
    gpu_mem_used: 8589934592,
    gpu_mem_total: 25769803776,
    ram_used: 10737418240,
    cpu_usage: 45.2,
    decoded_tokens: 1234,
    prompt_progress: 0.75,
    timestamp: 1234567890,
});

const parsed = parseWsMetrics(mockMetrics);
assert.strictEqual(parsed.model_name, 'llama3.gguf');
assert.strictEqual(parsed.loaded, true);
assert.strictEqual(parsed.tps, 42.5);
assert.strictEqual(parsed.prompt_tps, 1234.5);
assert.strictEqual(parsed.ctx_used, 2048);
assert.strictEqual(parsed.gpu_mem_total, 25769803776);
console.log('✓ WebSocket message parsing works');

// Test invalid JSON
const invalid = parseWsMetrics('not json');
assert.strictEqual(invalid, null);
console.log('✓ Invalid JSON returns null');

console.log('\n✓ All tests passed successfully!');
