const fs = require('node:fs');
const assert = require('node:assert');

 // Test buildWsUrl function
  function buildWsUrl(metricsUrl, secret) {
    try {
        const match = metricsUrl.match(/^(https?:)\/\/([^\/?#]+)(?:\?([^#]*))?/);
        if (!match) {
            throw new Error('Invalid URL format');
        }
        const protocol = match[1];
        const host = match[2];

        const wsProtocol = protocol === 'https:' ? 'wss' : 'ws';
        const wsUrl = `${wsProtocol}://${host}/ws`;
        const auth = secret || null;
        return { wsUrl, auth, hasAuth: !!auth };
    } catch (e) {
        return { wsUrl: 'ws://127.0.0.1:8080/ws', auth: null, hasAuth: false };
    }
}

console.log('Testing buildWsUrl...');

  // Test 1: Basic HTTP URL
    const result1 = buildWsUrl('http://127.0.0.1:8080/metrics', '');
assert.strictEqual(result1.wsUrl, 'ws://127.0.0.1:8080/ws');
assert.strictEqual(result1.hasAuth, false);
assert.strictEqual(result1.auth, null);
console.log('✓ Test 1 passed: Basic HTTP URL');

// Test 2: HTTPS URL
const result3 = buildWsUrl('https://example.com/metrics', '');
assert.strictEqual(result3.wsUrl, 'wss://example.com/ws');
assert.strictEqual(result3.hasAuth, false);
assert.strictEqual(result3.auth, null);
console.log('✓ Test 2 passed: HTTPS URL');

// Test 3: Secret as subprotocol (no URL query param)
const result4 = buildWsUrl('http://127.0.0.1:8080/metrics', 'mysecret');
assert.strictEqual(result4.wsUrl, 'ws://127.0.0.1:8080/ws');
assert.strictEqual(result4.hasAuth, true);
assert.strictEqual(result4.auth, 'mysecret');
console.log('✓ Test 3 passed: Secret returned as subprotocol');

// Test 4: Secret overrides URL query param
const result5 = buildWsUrl('http://127.0.0.1:8080/metrics?auth=oldsecret', 'newsecret');
assert.strictEqual(result5.wsUrl, 'ws://127.0.0.1:8080/ws');
assert.strictEqual(result5.hasAuth, true);
assert.strictEqual(result5.auth, 'newsecret');
console.log('✓ Test 4 passed: Secret overrides URL query param');

// Test 5: Invalid URL
const result7 = buildWsUrl('not-a-url', '');
assert.strictEqual(result7.wsUrl, 'ws://127.0.0.1:8080/ws');
assert.strictEqual(result7.hasAuth, false);
assert.strictEqual(result7.auth, null);
console.log('✓ Test 5 passed: Invalid URL fallback');

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

// Test truncateModelName function
function truncateModelName(name, maxLen) {
    if (!name) return 'No model';
    if (name.length <= maxLen) return name;
    return name.substring(0, maxLen);
}

console.log('\nTesting truncateModelName...');

assert.strictEqual(truncateModelName('my-model.gguf', 20), 'my-model.gguf');
assert.strictEqual(truncateModelName('my-model.gguf', 10), 'my-model.g');
assert.strictEqual(truncateModelName('a', 5), 'a');
assert.strictEqual(truncateModelName(undefined, 10), 'No model');
assert.strictEqual(truncateModelName('', 10), 'No model');
console.log('✓ truncateModelName tests passed');

// Test metric value formatting
const WS_METRICS = [
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

console.log('\nTesting WS_METRICS definition...');

assert.strictEqual(WS_METRICS.length, 11);
assert.strictEqual(WS_METRICS[0].key, 'model_name');
assert.strictEqual(WS_METRICS[0].type, 'text');
assert.strictEqual(WS_METRICS[4].type, 'ratio');
assert.strictEqual(WS_METRICS[4].used, 'ctx_used');
assert.strictEqual(WS_METRICS[5].type, 'ratio_gb');
assert.strictEqual(WS_METRICS[7].type, 'percent');
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

// Test Panel Ctx Percentage formatting vs Popdown value formatting
console.log('\nTesting Ctx display value formatting...');
const testCtxMetric = WS_METRICS.find(m => m.key === 'ctx');
assert.strictEqual(testCtxMetric.type, 'ratio');

function formatTokens(tokens) {
    if (tokens === undefined || tokens === null || isNaN(tokens)) return 'N/A';
    if (tokens >= 1024) return Math.floor(tokens / 1024) + 'K';
    return Math.round(tokens).toString();
}

// Mock metric formatter (representing the pop down menu display)
function testFormatMetricValue(metric, metrics) {
    if (metric.type === 'ratio') {
        const used = metrics[metric.used];
        const max = metrics[metric.max];
        if (used === undefined || max === undefined) return 'N/A';
        if (metric.unit === 'tokens') return `${formatTokens(used)} / ${formatTokens(max)}`;
        return `${used} / ${max}`;
    }
    return '-';
}

// Mock top bar display formatting
function testFormatTopBarValue(metric, metrics) {
    if (metric.type === 'ratio') {
        const used = metrics[metric.used];
        const max = metrics[metric.max];
        let percent = 0;
        if (used !== undefined && max !== undefined && max > 0) {
            percent = Math.round((used / max) * 100);
        }
        if (metric.key === 'ctx') {
            return (used !== undefined && max !== undefined && max > 0) ? `${percent}%` : 'N/A';
        }
        return testFormatMetricValue(metric, metrics);
    }
    return '-';
}

const mockState1 = { ctx_used: 2048, ctx_max: 8192 };
assert.strictEqual(testFormatMetricValue(testCtxMetric, mockState1), '2K / 8K');
assert.strictEqual(testFormatTopBarValue(testCtxMetric, mockState1), '25%');

const mockState2 = { ctx_used: undefined, ctx_max: 8192 };
assert.strictEqual(testFormatMetricValue(testCtxMetric, mockState2), 'N/A');
assert.strictEqual(testFormatTopBarValue(testCtxMetric, mockState2), 'N/A');

console.log('✓ Ctx display value formatting tests passed');

console.log('\n✓ All tests passed successfully!');
