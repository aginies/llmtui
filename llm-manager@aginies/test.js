const fs = require('node:fs');
const assert = require('node:assert');
const vm = require('node:vm');

// Load and extract the pure JavaScript code from extension.js
const code = fs.readFileSync('/home/aginies/.local/share/gnome-shell/extensions/llm-manager@siri/extension.js', 'utf8');

// We extract the array and functions using simple regex or direct evaluation in a VM context.
// Rather than fragile regex, we can replace the GJS imports with mocks and run the code!
// Let's create a mock environment for GJS imports.
const mockClutter = {};
const mockGio = {
    Subprocess: {
        new: () => ({})
    },
    SubprocessFlags: {}
};
const mockGLib = {
    timeout_add_seconds: () => {},
    Source: { remove: () => {} },
    PRIORITY_DEFAULT: 0
};
const mockGObject = {
    registerClass: (meta, cls) => cls
};
const mockSt = {
    BoxLayout: class {},
    Icon: class {},
    Label: class {}
};

// Create a context with our mocks
const sandbox = {
    Clutter: mockClutter,
    Gio: mockGio,
    GLib: mockGLib,
    GObject: mockGObject,
    St: mockSt,
    console: {
        log: () => {}, // Suppress console logs during test unless needed
        error: console.error
    },
};

// Let's strip the ESM imports and exports so it can be evaluated as a standard script in VM
const cleanCode = code
    .replace(/import\s+Clutter\s+from\s+['"]gi:\/\/Clutter['"];/, 'const Clutter = St.BoxLayout;') // already mocked
    .replace(/import\s+Gio\s+from\s+['"]gi:\/\/Gio['"];/, '')
    .replace(/import\s+GLib\s+from\s+['"]gi:\/\/GLib['"];/, '')
    .replace(/import\s+GObject\s+from\s+['"]gi:\/\/GObject['"];/, '')
    .replace(/import\s+St\s+from\s+['"]gi:\/\/St['"];/, '')
    .replace(/import\s+\*\s+as\s+PanelMenu\s+from\s+['"][^'"]+['"];/, 'const PanelMenu = { Button: class {} };')
    .replace(/import\s+\*\s+as\s+PopupMenu\s+from\s+['"][^'"]+['"];/, 'const PopupMenu = { PopupSeparatorMenuItem: class {}, PopupMenuItem: class {}, PopupBaseMenuItem: class {} };')
    .replace(/import\s+\*\s+as\s+Main\s+from\s+['"][^'"]+['"];/, 'const Main = { panel: { _leftBox: { insert_child_at_index: () => {} } } };')
    .replace(/import\s+\{[^}]+\}\s+from\s+['"]resource:\/\/\/org\/gnome\/shell\/extensions\/extension.js['"];/, 'const Extension = class {}; const _ = (x) => x;')
    .replace(/export\s+default\s+class/, 'class');

vm.createContext(sandbox);
vm.runInContext(cleanCode, sandbox);

// Extract the parsed functions/constants from sandbox
const {
    parsePrometheusMetrics,
    formatNumber,
    formatCounter,
    getMetricColor,
    PROMETHEUS_METRICS
} = sandbox;

// Run the tests!
console.log('Running LLM Manager Extension Tests...');

// 1. Test parsePrometheusMetrics with the user's metrics payload
const mockMetricsPayload = `
# HELP llamacpp:prompt_tokens_total Number of prompt tokens processed.
# TYPE llamacpp:prompt_tokens_total counter
llamacpp:prompt_tokens_total 479209
# HELP llamacpp:prompt_seconds_total Prompt process time
# TYPE llamacpp:prompt_seconds_total counter
llamacpp:prompt_seconds_total 145.713
# HELP llamacpp:tokens_predicted_total Number of generation tokens processed.
# TYPE llamacpp:tokens_predicted_total counter
llamacpp:tokens_predicted_total 123063
# HELP llamacpp:tokens_predicted_seconds_total Predict process time
# TYPE llamacpp:tokens_predicted_seconds_total counter
llamacpp:tokens_predicted_seconds_total 683.744
# HELP llamacpp:n_decode_total Total number of llama_decode() calls
# TYPE llamacpp:n_decode_total counter
llamacpp:n_decode_total 43512
# HELP llamacpp:n_tokens_max Largest observed n_tokens.
# TYPE llamacpp:n_tokens_max counter
llamacpp:n_tokens_max 82854
# HELP llamacpp:prompt_tokens_seconds Average prompt throughput in tokens/s.
# TYPE llamacpp:prompt_tokens_seconds gauge
llamacpp:prompt_tokens_seconds 3288.72
# HELP llamacpp:predicted_tokens_seconds Average generation throughput in tokens/s.
# TYPE llamacpp:predicted_tokens_seconds gauge
llamacpp:predicted_tokens_seconds 179.984
# HELP llamacpp:requests_processing Number of requests processing.
# TYPE llamacpp:requests_processing gauge
llamacpp:requests_processing 0
# HELP llamacpp:requests_deferred Number of requests deferred.
# TYPE llamacpp:requests_deferred gauge
llamacpp:requests_deferred 0
# HELP llamacpp:n_busy_slots_per_decode Average number of busy slots per llama_decode() call
# TYPE llamacpp:n_busy_slots_per_decode gauge
llamacpp:n_busy_slots_per_decode 1.12925
`;

const parsed = parsePrometheusMetrics(mockMetricsPayload);
assert.strictEqual(parsed.prompt_tokens_total, 479209);
assert.strictEqual(parsed.prompt_seconds_total, 145.713);
assert.strictEqual(parsed.tokens_predicted_total, 123063);
assert.strictEqual(parsed.tokens_predicted_seconds_total, 683.744);
assert.strictEqual(parsed.n_decode_total, 43512);
assert.strictEqual(parsed.n_tokens_max, 82854);
assert.strictEqual(parsed.prompt_tokens_seconds, 3288.72);
assert.strictEqual(parsed.predicted_tokens_seconds, 179.984);
assert.strictEqual(parsed.requests_processing, 0);
assert.strictEqual(parsed.requests_deferred, 0);
assert.strictEqual(parsed.n_busy_slots_per_decode, 1.12925);
console.log('✓ parsePrometheusMetrics parsed all metrics successfully!');

// 2. Test formatNumber
assert.strictEqual(formatNumber(12.3456, 2), '12.35');
assert.strictEqual(formatNumber(12.3456, 0), '12');
assert.strictEqual(formatNumber(undefined, 2), 'N/A');
assert.strictEqual(formatNumber(null, 2), 'N/A');
assert.strictEqual(formatNumber(NaN, 2), 'N/A');
console.log('✓ formatNumber verified successfully!');

// 3. Test formatCounter
assert.strictEqual(formatCounter(123, 's'), '123s');
assert.strictEqual(formatCounter(1234, 's'), '1.2Ks');
assert.strictEqual(formatCounter(1234567, ''), '1.23M');
assert.strictEqual(formatCounter(undefined, 's'), 'N/A');
console.log('✓ formatCounter verified successfully!');

// 4. Test getMetricColor
assert.strictEqual(getMetricColor(0, 'requests_processing'), 'llm-value-good');
assert.strictEqual(getMetricColor(2, 'requests_processing'), 'llm-value-warn');
assert.strictEqual(getMetricColor(4, 'requests_processing'), 'llm-value-bad');
assert.strictEqual(getMetricColor(1.0, 'n_busy_slots_per_decode'), 'llm-value-good');
assert.strictEqual(getMetricColor(0.7, 'n_busy_slots_per_decode'), 'llm-value-warn');
assert.strictEqual(getMetricColor(0.4, 'n_busy_slots_per_decode'), 'llm-value-bad');
console.log('✓ getMetricColor verified successfully!');

console.log('All tests passed successfully!');
