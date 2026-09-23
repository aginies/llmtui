/**
 * Pi Orchestrator Extension
 *
 * A custom tool that ALWAYS runs the task on the remote llama.cpp server
 * configured in orchestrator.json (or env vars). No local subprocess is
 * ever spawned — the task (plus optional attached files) is sent to the
 * server's OpenAI-compatible API as a chat completion.
 *
 *   Sync mode (default):
 *     The task is sent to the remote server and the tool waits for the
 *     answer, returning it as the tool result.
 *
 *   Async mode (async: true):
 *     The request is fired in the background and the tool returns
 *     immediately. When the server responds, the answer is delivered to
 *     the session once the local session is idle.
 *
 *
 * Config (orchestrator.json in ~/.pi/ or .pi/, or ORCHESTRATOR_* env vars):
 *   { "llamaUrl": "http://remote-host:8080", "llamaModel": "qwen2.5:7b" }
 *
 * Usage (LLM calls via the `orchestrate` tool):
 *   orchestrate(
 *     task: "Explain the trade-offs between REST and gRPC",
 *     sendFiles: ["src/api.ts"]   // optional: attach file contents
 *   )
 */

import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { execFile, spawn } from "node:child_process";
import {
	CONFIG_DIR_NAME,
	type ExtensionAPI,
	type ExtensionContext,
	getMarkdownTheme,
} from "@earendil-works/pi-coding-agent";
import { Container, Markdown, Spacer, Text } from "@earendil-works/pi-tui";
import { Type } from "typebox";

// ─── Constants ───────────────────────────────────────────────────────────────

const DEFAULT_MAX_PROMPT_LENGTH = 64 * 1024; // 64KB
const DEFAULT_MAX_FILE_CONTENT = 32 * 1024; // 32KB per attached file
const DEFAULT_DIFF_MAX_SIZE = 32 * 1024; // 32KB for the git diff attachment
const DIFF_TIMEOUT_MS = 10_000; // 10s
const LLAMA_TIMEOUT_MS = 300_000; // 300s
const UPLOAD_TIMEOUT_MS = 600_000; // 600s — tarball upload over the wire
const AGENT_TIMEOUT_MS = 600_000; // 600s — the remote agent runs multiple LLM rounds
const MAX_TARBALL_SIZE = 512 * 1024 * 1024; // 512 MiB hard cap on sendDir tarballs
const TAR_EXCLUDES = [
	".git",
	"node_modules",
	"target",
	"dist",
	"build",
	".venv",
	"__pycache__",
];
const PING_TIMEOUT_MS = 5_000; // 5s
const MIN_ATTACHMENT_CHUNK = 4 * 1024; // never include a file fragment smaller than this
const IDLE_POLL_INTERVAL_MS = 500; // Poll for idle state every 500ms
const IDLE_POLL_TIMEOUT_MS = 10 * 60 * 1000; // 10 min max wait for idle
const CUSTOM_MESSAGE_TYPE = "orchestrator:result";
const STATUS_ENTRY_TYPE = "orchestrator:status";
const MAX_TRACKED_TASKS = 50;

/** Estimate total uncompressed size of a directory (excluding the same patterns as TAR_EXCLUDES). */
function estimateDirSize(dir: string): number {
	let total = 0;
	const entries = fs.readdirSync(dir, { recursive: true });
	for (const entry of entries) {
		const rel = entry as string;
		const parts = rel.split(path.sep);
		const shouldExclude = parts.some((p) => TAR_EXCLUDES.includes(p));
		if (shouldExclude) continue;
		const full = path.join(dir, rel);
		try {
			const stat = fs.statSync(full);
			if (stat.isFile()) total += stat.size;
		} catch {
			// skip unreadable entries
		}
	}
	return total;
}

/** Recursively list all files in a directory (excluding the same patterns as TAR_EXCLUDES). */
function dirToFiles(dir: string): string[] {
	const files: string[] = [];
	const entries = fs.readdirSync(dir, { recursive: true });
	for (const entry of entries) {
		const rel = entry as string;
		const parts = rel.split(path.sep);
		const shouldExclude = parts.some((p) => TAR_EXCLUDES.includes(p));
		if (shouldExclude) continue;
		const full = path.join(dir, rel);
		try {
			if (fs.statSync(full).isFile()) files.push(rel);
		} catch {
			// skip unreadable entries
		}
	}
	return files;
}

/** Retry a function with exponential backoff on transient errors. */
async function withRetry<T>(
	fn: () => Promise<T>,
	maxRetries: number,
	baseMs: number,
	signal?: AbortSignal,
): Promise<T> {
	let lastErr: Error | undefined;
	for (let attempt = 0; attempt <= maxRetries; attempt++) {
		try {
			return await fn();
		} catch (err) {
			lastErr = err as Error;
			// Don't retry on abort
			if (signal?.aborted) throw lastErr;
			// Retry only on transient errors (network, timeouts, 5xx)
			const msg = lastErr.message.toLowerCase();
			const isTransient =
				msg.includes("fetch") ||
				msg.includes("timeout") ||
				msg.includes("ECONNREFUSED") ||
				msg.includes("ECONNRESET") ||
				msg.includes("ETIMEDOUT") ||
				msg.includes("EPIPE") ||
				msg.includes("502") ||
				msg.includes("503") ||
				msg.includes("504");
			if (!isTransient || attempt === maxRetries) throw lastErr;
			const delay = baseMs * (1 << attempt);
			await new Promise((r) => setTimeout(r, delay));
		}
	}
	throw lastErr!;
}

/**
 * Resolve a path and verify it stays within `base`. Returns the resolved
 * path or throws — used to prevent path-traversal in sendDir/sendFiles.
 */
function safeResolve(base: string, rel: string): string {
	const resolved = path.resolve(base, rel);
	if (!resolved.startsWith(path.resolve(base) + path.sep) && resolved !== path.resolve(base)) {
		throw new Error(`path escapes the allowed directory: ${rel}`);
	}
	return resolved;
}

// ─── Configuration ───────────────────────────────────────────────────────────

interface OrchestratorConfig {
	llamaUrl: string;
	llamaModel: string;
	llamaSystemPrompt: string;
	llamaApiKey: string;
	llamaMaxTokens: number;
	llamaTemperature: number;
	llamaTopP: number;
	maxPromptLength: number;
	maxFileContent: number;
	diffMaxSize: number;
}

function readJsonConfig(
	configPath: string,
): Partial<OrchestratorConfig> | undefined {
	if (!fs.existsSync(configPath)) return undefined;
	try {
		return JSON.parse(
			fs.readFileSync(configPath, "utf-8"),
		) as Partial<OrchestratorConfig>;
	} catch {
		return undefined;
	}
}

function loadConfig(): OrchestratorConfig {
	const env = process.env;

	// Layered config: hardcoded defaults < env vars < user config
	// (~/.pi/orchestrator.json) < project config (.pi/orchestrator.json)
	const envConfig: OrchestratorConfig = {
		llamaUrl: env.ORCHESTRATOR_LLAMA_URL || "http://localhost:8080",
		llamaModel: env.ORCHESTRATOR_LLAMA_MODEL || "qwen2.5:7b",
		llamaSystemPrompt: env.ORCHESTRATOR_LLAMA_SYSTEM_PROMPT || "",
		llamaApiKey: env.ORCHESTRATOR_LLAMA_API_KEY || "",
		llamaMaxTokens: Number(env.ORCHESTRATOR_LLAMA_MAX_TOKENS) || 4096,
		llamaTemperature: Number(env.ORCHESTRATOR_LLAMA_TEMPERATURE) || 0.7,
		llamaTopP: Number(env.ORCHESTRATOR_LLAMA_TOP_P) || 0.9,
		maxPromptLength:
			Number(env.ORCHESTRATOR_MAX_PROMPT_LENGTH) || DEFAULT_MAX_PROMPT_LENGTH,
		maxFileContent:
			Number(env.ORCHESTRATOR_MAX_FILE_CONTENT) || DEFAULT_MAX_FILE_CONTENT,
		diffMaxSize:
			Number(env.ORCHESTRATOR_DIFF_MAX_SIZE) || DEFAULT_DIFF_MAX_SIZE,
	};

	const userConfig = readJsonConfig(
		path.join(os.homedir(), CONFIG_DIR_NAME, "orchestrator.json"),
	);
	const projectConfig = readJsonConfig(
		path.join(process.cwd(), CONFIG_DIR_NAME, "orchestrator.json"),
	);

	const merged: OrchestratorConfig = { ...envConfig };
	for (const layer of [userConfig, projectConfig]) {
		if (!layer) continue;
		for (const [key, value] of Object.entries(layer)) {
			if (value !== undefined && value !== null && value !== "") {
				(merged as Record<string, unknown>)[key] = value;
			}
		}
	}
	return merged;
}

// ─── Llama.cpp client ────────────────────────────────────────────────────────

interface LlamaCppConfig {
	url: string;
	model: string;
	systemPrompt: string;
	maxTokens: number;
	temperature: number;
	topP: number;
	maxPromptLength: number;
	maxFileContent: number;
}

interface LlamaResponse {
	content: string;
	usage?: { input: number; output: number };
	// Transfer-mode extras (set when the task ran via tarball + remote agent)
	transferId?: string;
	rounds?: number;
	filesRead?: string[];
}

async function chatCompletion(
	llamaConfig: LlamaCppConfig,
	userContent: string,
	signal: AbortSignal | undefined,
): Promise<LlamaResponse> {
	return withRetry(
		() => _chatCompletion(llamaConfig, userContent, signal),
		2, // max 2 retries (3 total attempts)
		1000, // 1s base delay
		signal,
	);
}

async function _chatCompletion(
	llamaConfig: LlamaCppConfig,
	userContent: string,
	signal: AbortSignal | undefined,
): Promise<LlamaResponse> {
	const baseUrl = llamaConfig.url.replace(/\/+$/, "");
	const endpoint = `${baseUrl}/v1/chat/completions`;

	const messages: Array<{ role: string; content: string }> = [];

	// Add system prompt if configured
	if (llamaConfig.systemPrompt) {
		messages.push({ role: "system", content: llamaConfig.systemPrompt });
	}

	// Cap prompt length to prevent DoS / OOM on the LLM server
	if (userContent.length > llamaConfig.maxPromptLength) {
		userContent =
			userContent.slice(0, llamaConfig.maxPromptLength) +
			"\n\n... [truncated, max " +
			llamaConfig.maxPromptLength +
			" chars]";
	}

	messages.push({ role: "user", content: userContent });

	const payload = {
		model: llamaConfig.model,
		messages,
		stream: false,
		max_tokens: llamaConfig.maxTokens,
		temperature: llamaConfig.temperature,
		top_p: llamaConfig.topP,
	};

	// AbortController for fetch timeout
	const fetchController = new AbortController();
	const timeoutId = setTimeout(() => fetchController.abort(), LLAMA_TIMEOUT_MS);

	try {
		const response = await fetch(endpoint, {
			method: "POST",
			headers: {
				"Content-Type": "application/json",
			},
			body: JSON.stringify(payload),
			signal: signal || fetchController.signal,
		});

		if (!response.ok) {
			const errorText = await response.text();
			throw new Error(
				`llama.cpp request failed (${response.status}): ${errorText}`,
			);
		}

		const data = (await response.json()) as {
			choices: Array<{ message: { content: string }; finish_reason: string }>;
			usage?: { prompt_tokens: number; completion_tokens: number };
		};

		const content = data.choices?.[0]?.message?.content ?? "(no response)";
		const usage = data.usage
			? {
					input: data.usage.prompt_tokens,
					output: data.usage.completion_tokens,
				}
			: undefined;

		return { content, usage };
	} finally {
		clearTimeout(timeoutId);
	}
}

// ─── Transfer + remote agent client ─────────────────────────────────────────

interface TransferUploadResult {
	id: string;
	path: string;
	files: Array<{ path: string; size: number }>;
}

interface AgentRunResult {
	answer: string;
	rounds: number;
	files_read: string[];
}

/**
 * Combine an optional caller AbortSignal with a per-request timeout into a
 * single signal. Returns the signal and a `done()` to clear the timer.
 */
function withTimeout(
	signal: AbortSignal | undefined,
	ms: number,
): { signal: AbortSignal; done: () => void } {
	const controller = new AbortController();
	const timeoutId = setTimeout(() => controller.abort(), ms);
	if (signal) {
		if (signal.aborted) controller.abort();
		else signal.addEventListener("abort", () => controller.abort(), { once: true });
	}
	return {
		signal: controller.signal,
		done: () => clearTimeout(timeoutId),
	};
}

/**
 * Tar a directory in memory: `tar czf - --exclude=... -C <dir> .`
 * Standard VCS/dependency/build dirs are excluded so the remote agent gets
 * source, not noise.
 */
function buildTarball(dir: string): Promise<Buffer> {
	return new Promise((resolve, reject) => {
		const args = ["czf", "-"];
		for (const ex of TAR_EXCLUDES) args.push(`--exclude=${ex}`);
		args.push("-C", dir, ".");
		const proc = spawn("tar", args);
		const chunks: Buffer[] = [];
		let stderr = "";
		proc.stdout.on("data", (c: Buffer) => chunks.push(c));
		proc.stderr.on("data", (c: Buffer) => {
			stderr += c.toString();
		});
		proc.on("error", (err) => reject(new Error(`tar failed: ${err.message}`)));
		proc.on("close", (code) => {
			if (code !== 0) {
				reject(
					new Error(
						`tar exited ${code}: ${stderr.trim().split("\n")[0] || "unknown error"}`,
					),
				);
			} else {
				resolve(Buffer.concat(chunks));
			}
		});
	});
}

/**
 * POST the tarball to the remote llm-manager's transfer API. Returns the
 * transfer id (plus the extraction path and file manifest).
 */
async function uploadTransfer(
	baseUrl: string,
	apiKey: string,
	tarball: Buffer,
	label: string,
	signal: AbortSignal | undefined,
): Promise<TransferUploadResult> {
	const t = withTimeout(signal, UPLOAD_TIMEOUT_MS);
	try {
		const response = await withRetry(
			() => _uploadTransfer(baseUrl, apiKey, tarball, label),
			2,
			1000,
			t.signal,
		);
		return response;
	} finally {
		t.done();
	}
}

async function _uploadTransfer(
	baseUrl: string,
	apiKey: string,
	tarball: Buffer,
	label: string,
): Promise<TransferUploadResult> {
	const response = await fetch(
		`${baseUrl}/api/transfer/upload?label=${encodeURIComponent(label)}`,
		{
			method: "POST",
			headers: {
				"Content-Type": "application/gzip",
				Authorization: `Bearer ${apiKey}`,
			},
			body: tarball,
		},
	);
	if (!response.ok) {
		const errorText = await response.text();
		throw new Error(
			`transfer upload failed (${response.status}): ${errorText}`,
		);
	}
	return (await response.json()) as TransferUploadResult;
}

/**
 * POST /api/agent/run — run the remote review agent over the uploaded
 * transfer. The remote model pages through the files itself and returns
 * its final answer.
 */
async function runAgent(
	baseUrl: string,
	apiKey: string,
	transferId: string,
	task: string,
	signal: AbortSignal | undefined,
): Promise<AgentRunResult> {
	const t = withTimeout(signal, AGENT_TIMEOUT_MS);
	try {
		const response = await fetch(`${baseUrl}/api/agent/run`, {
			method: "POST",
			headers: {
				"Content-Type": "application/json",
				Authorization: `Bearer ${apiKey}`,
			},
			body: JSON.stringify({ transfer_id: transferId, task }),
			signal: t.signal,
		},
		);
		if (!response.ok) {
			const errorText = await response.text();
			throw new Error(`agent run failed (${response.status}): ${errorText}`);
		}
		return (await response.json()) as AgentRunResult;
	} finally {
		t.done();
	}
}

// ─── Health check ────────────────────────────────────────────────────────────

interface PingResult {
	ok: boolean;
	latencyMs?: number;
	error?: string;
}

/**
 * GET <url>/health with a short timeout. Used by `/orchestrator ping` and as
 * a pre-flight before the first remote call so a dead server fails fast
 * instead of hanging for the full request timeout.
 */
async function pingServer(url: string): Promise<PingResult> {
	const baseUrl = url.replace(/\/+$/, "");
	const controller = new AbortController();
	const timeoutId = setTimeout(() => controller.abort(), PING_TIMEOUT_MS);
	const start = Date.now();
	try {
		const response = await fetch(`${baseUrl}/health`, {
			signal: controller.signal,
		});
		const latencyMs = Date.now() - start;
		if (!response.ok) {
			return { ok: false, latencyMs, error: `HTTP ${response.status}` };
		}
		return { ok: true, latencyMs };
	} catch (err) {
		const message = (err as Error).name === "AbortError"
			? `timeout after ${PING_TIMEOUT_MS / 1000}s`
			: (err as Error).message;
		return { ok: false, error: message };
	} finally {
		clearTimeout(timeoutId);
	}
}

// ─── Git diff attachment ─────────────────────────────────────────────────────

type DiffMode = "unstaged" | "staged" | "all";

const DIFF_COMMANDS: Record<DiffMode, string[]> = {
	unstaged: ["diff"],
	staged: ["diff", "--staged"],
	all: ["diff", "HEAD"],
};

/**
 * Run `git diff` (variant per mode) in cwd. Resolves to the diff text, or an
 * inline note `(git … failed: …)` / `(no … changes)` — never rejects, so a
 * missing repo or empty worktree degrades to a note instead of failing the call.
 */
function runGitDiff(cwd: string, mode: DiffMode): Promise<string> {
	const args = DIFF_COMMANDS[mode];
	return new Promise((resolve) => {
		execFile(
			"git",
			args,
			{ cwd, timeout: DIFF_TIMEOUT_MS, maxBuffer: 16 * 1024 * 1024 },
			(err, stdout, stderr) => {
				if (err) {
					const msg = (stderr || err.message).toString().trim().split("\n")[0];
					resolve(`(git ${args.join(" ")} failed: ${msg})`);
				} else {
					resolve(stdout);
				}
			},
		);
	});
}

/**
 * Build the `--- Changes (git diff: …) ---` prompt section, capped at maxSize.
 */
async function buildDiffSection(
	cwd: string,
	mode: DiffMode,
	maxSize: number,
): Promise<string> {
	const diff = await runGitDiff(cwd, mode);
	const header = `--- Changes (git diff: ${mode}) ---`;
	if (!diff.trim()) return `${header}\n(no ${mode} changes)`;
	let body = diff;
	if (diff.length > maxSize) {
		body = diff.slice(0, maxSize) + `\n... [diff truncated, max ${maxSize} bytes]`;
	}
	return `${header}\n${body}`;
}

// ─── File attachments ────────────────────────────────────────────────────────

function formatKb(bytes: number): string {
	return `${(bytes / 1024).toFixed(1)} KB`;
}

interface AttachmentBudget {
	block: string;
	manifest: string | null;
}

/**
 * A sendFiles entry: a path, optionally with a 1-based inclusive line range
 * (`src/foo.ts:10-120` or `src/foo.ts:42`).
 */
interface FileEntrySpec {
	file: string;
	start?: number;
	end?: number;
}

function parseFileEntry(entry: string): FileEntrySpec {
	const m = /^(.*?):(\d+)(?:-(\d+))?$/s.exec(entry);
	if (!m) return { file: entry };
	let start = Number(m[2]);
	let end = m[3] !== undefined ? Number(m[3]) : start;
	if (start > end) [start, end] = [end, start]; // tolerate reversed ranges
	return { file: m[1], start, end };
}

/**
 * Read the given files and format them as an attachment block that is appended
 * to the prompt sent to the remote server, so the (tool-less) remote model
 * can see the actual code. Relative paths resolve against baseCwd.
 *
 * Budget allocator: files are allocated against `budget` (chars) in the given
 * order — whole files while they fit, the next file truncated if a useful
 * chunk remains, the rest dropped. Each file is still capped at
 * maxFileContent. A manifest line reports what was truncated/dropped so
 * nothing is lost silently.
 */
function buildFileAttachments(
	files: string[],
	baseCwd: string,
	maxFileContent: number,
	budget: number,
): AttachmentBudget {
	const parts: string[] = [];
	const truncated: string[] = [];
	const dropped: string[] = [];
	let overBudget = 0;
	let remaining = budget;

	for (const entry of files) {
		const spec = parseFileEntry(entry);
		const file = spec.file;
		let abs: string;
		try {
			abs = safeResolve(baseCwd, file);
		} catch {
			parts.push(`### ${entry}\n(path-traversal blocked)`);
			continue;
		}
		let content: string;
		try {
			content = fs.readFileSync(abs, "utf-8");
		} catch (err) {
			const note = `### ${entry}\n(could not read: ${(err as Error).message})`;
			parts.push(note);
			remaining -= note.length;
			continue;
		}
		if (content.includes("\u0000")) {
			const note = `### ${entry}\n(binary file, skipped)`;
			parts.push(note);
			remaining -= note.length;
			continue;
		}

		// Line-range selection: only the selected lines count against the budget
		let label = file;
		if (spec.start !== undefined) {
			const lines = content.split("\n");
			const total = lines.length;
			const start = Math.max(1, spec.start);
			const end = Math.min(total, spec.end ?? spec.start);
			if (start > end) {
				const note = `### ${entry}\n(no lines: requested ${spec.start}\u2013${spec.end}, file has ${total} lines)`;
				parts.push(note);
				remaining -= note.length;
				continue;
			}
			content = lines.slice(start - 1, end).join("\n");
			if (start !== 1 || end !== total) {
				const clamped = spec.start! < start || (spec.end ?? spec.start) > end;
				label = `${file} (lines ${start}\u2013${end} of ${total}${clamped ? `, requested ${spec.start}\u2013${spec.end}` : ""})`;
			}
		}

		const totalSize = content.length;
		const size = Math.min(totalSize, maxFileContent); // per-file cap
		// header + code fences + join separator
		const overhead = `### ${label}\n\`\`\`\n\n\`\`\`\n\n`.length;

		if (size + overhead <= remaining) {
			// Fits whole (subject to the per-file cap)
			let body = content;
			if (totalSize > maxFileContent) {
				body =
					content.slice(0, maxFileContent) +
					`\n... [truncated, max ${maxFileContent} bytes]`;
			}
			parts.push(`### ${label}\n\`\`\`\n${body}\n\`\`\``);
			remaining -= body.length + overhead;
			continue;
		}

		// Doesn't fit whole — truncate to what the budget allows, if useful
		const reserve = 48; // room for the truncation marker
		const cut = remaining - overhead - reserve;
		if (cut >= MIN_ATTACHMENT_CHUNK) {
			const marker = `\n... [truncated: ${formatKb(cut)} of ${formatKb(size)}]`;
			parts.push(`### ${label}\n\`\`\`\n${content.slice(0, cut)}${marker}\n\`\`\``);
			remaining -= cut + marker.length + overhead;
			truncated.push(`${label} ${formatKb(cut)}/${formatKb(size)}`);
			overBudget += size - cut;
		} else {
			dropped.push(label);
			overBudget += size;
		}
	}

	let manifest: string | null = null;
	if (truncated.length > 0 || dropped.length > 0) {
		const bits: string[] = [];
		if (truncated.length > 0) {
			bits.push(`${truncated.length} truncated (${truncated.join(", ")})`);
		}
		if (dropped.length > 0) {
			bits.push(`${dropped.length} dropped (${dropped.join(", ")})`);
		}
		manifest = `[attachments: ${bits.join(", ")}; ${formatKb(overBudget)} over budget]`;
	}

	return { block: parts.join("\n\n"), manifest };
}

/**
 * Build the prompt sent to the remote server: the task plus any attached
 * file contents, allocated against maxPromptLength so the prompt never
 * exceeds it (the chatCompletion cap stays as a last-resort safety net).
 */
function buildPrompt(
	task: string,
	files: string[] | undefined,
	baseCwd: string,
	maxFileContent: number,
	maxPromptLength: number,
	diffSection?: string,
): string {
	const diffPrefix = diffSection ? `\n\n${diffSection}` : "";
	if (!files?.length) return `${task}${diffPrefix}`;
	const header = "\n\n--- Attached Files ---\n";
	const baseBudget = Math.max(
		0,
		maxPromptLength - task.length - diffPrefix.length - header.length,
	);

	let budget = baseBudget;
	let result = buildFileAttachments(files, baseCwd, maxFileContent, budget);

	// The manifest is appended after allocation, so it is not counted against
	// the budget. If it pushes the prompt over the cap, shrink the budget by
	// the overshoot and allocate once more (converges: a smaller budget only
	// adds manifest entries, never removes the need for them).
	const manifestLen = result.manifest ? `\n\n${result.manifest}`.length : 0;
	const overshoot =
		task.length +
		diffPrefix.length +
		header.length +
		result.block.length +
		manifestLen -
		maxPromptLength;
	if (overshoot > 0) {
		budget = Math.max(0, baseBudget - overshoot);
		result = buildFileAttachments(files, baseCwd, maxFileContent, budget);
	}

	let prompt = `${task}${diffPrefix}${header}${result.block}`;
	if (result.manifest) prompt += `\n\n${result.manifest}`;
	return prompt;
}

// ─── Background task tracking ────────────────────────────────────────────────

interface TrackedTask {
	taskId: string;
	task: string;
	model: string;
	startedAt: number;
	state: "running" | "done" | "failed" | "killed";
	finishedAt?: number;
	error?: string;
}

// ─── Tool registration ───────────────────────────────────────────────────────

const OrchestratorParams = Type.Object({
	task: Type.String({
		description:
			"The task to run on the remote llama.cpp server. Only call this tool when the user explicitly asks for orchestration (e.g. their message contains the word 'orchestrate' or 'pi-orch').",
	}),
	llamaUrl: Type.Optional(
		Type.String({
			description:
				"Remote llama.cpp server URL (e.g. http://remote-host:8080). Falls back to config/env.",
		}),
	),
	llamaModel: Type.Optional(
		Type.String({
			description: "Model name on the remote server. Falls back to config/env.",
		}),
	),
	llamaSystemPrompt: Type.Optional(
		Type.String({
			description:
				"System prompt for the remote reasoning round. Falls back to config/env.",
		}),
	),
	llamaApiKey: Type.Optional(
		Type.String({
			description:
				"Bearer API key for the remote llm-manager (required for sendDir transfer mode). Falls back to config/env.",
		}),
	),
	llamaMaxTokens: Type.Optional(
		Type.Number({
			description: "Max tokens for the remote response. Falls back to config/env.",
		}),
	),
	llamaTemperature: Type.Optional(
		Type.Number({
			description: "Temperature (0-2). Falls back to config/env.",
		}),
	),
	llamaTopP: Type.Optional(
		Type.Number({
			description: "Top-p (0-1). Falls back to config/env.",
		}),
	),
	cwd: Type.Optional(
		Type.String({
			description:
				"Base directory for resolving relative sendFiles paths. Defaults to current project.",
		}),
	),
	sendFiles: Type.Optional(
		Type.Array(Type.String(), {
			description:
				"File paths to attach: their contents are appended to the prompt sent to the remote server, so the remote model can see the actual code (it has no file access). Relative paths resolve against cwd. Line ranges are supported: 'src/foo.ts:10-120' or 'src/foo.ts:42' (1-based, inclusive) — only the selected lines are sent. Files are allocated against maxPromptLength in the given order: whole files while they fit, then truncated or dropped — a manifest line reports what was cut.",
		}),
	),
	async: Type.Optional(
		Type.Boolean({
			description:
				"Fire the request in the background and return immediately. The remote answer is delivered to the session when it arrives (waits for the session to be idle first). Default (false): wait for the remote answer and return it as the tool result.",
		}),
	),
	sendDir: Type.Optional(
		Type.String({
			description:
				"Transfer mode: tar this directory (default: cwd), upload it to the remote llm-manager's transfer API, and run its review agent over the files — the remote model reads the files itself, so there is no prompt-size limit. Requires llamaApiKey (config or param). Use this for whole-project reviews instead of sendFiles.",
		}),
	),
	sendDiff: Type.Optional(
		Type.Union([Type.Boolean(), Type.Enum(["unstaged", "staged", "all"])], {
			description:
				"Attach the current git diff to the prompt: true = unstaged changes, or 'staged' / 'all' (staged + unstaged vs HEAD). The natural choice for 'review my changes'. Default false.",
		}),
	),
});

export default function (pi: ExtensionAPI) {
	const config = loadConfig();

	// ── EventBus listener: receives async results and delivers to session ──
	let currentCtx: ExtensionContext | undefined;

	// Registry of background tasks for `/orchestrator status`
	const tasks = new Map<string, TrackedTask>();
	// Per-task abort controllers for `/orchestrator kill`
	const abortControllers = new Map<string, AbortController>();
	// URL that last passed the pre-flight ping (re-ping if it changes)
	let lastPingedUrl: string | undefined;

	const handleAsyncResult = (data: unknown) => {
		const result = data as {
			taskId: string;
			error?: string;
			response?: LlamaResponse;
			llamaConfig: LlamaCppConfig;
		};

		// Clean up the task entry regardless of whether we can deliver.
		// This prevents the "running" entry from staying in the map forever
		// if the session ended (currentCtx cleared) before delivery.
		const entry = tasks.get(result.taskId);
		if (entry && entry.state === "running") {
			entry.state = result.error ? "failed" : "done";
			entry.finishedAt = Date.now();
			if (result.error) entry.error = result.error;
			// Also clean the abort controller if it still exists
			abortControllers.delete(result.taskId);
		}

		const ctx = currentCtx;
		if (!ctx) {
			console.error(
				"[orchestrator] No context available, task entry cleaned up but result dropped",
			);
			return;
		}

		// Wait for session to be idle before delivering
		const deliverResult = async () => {
			const deadline = Date.now() + IDLE_POLL_TIMEOUT_MS;
			while (Date.now() < deadline) {
				if (ctx.isIdle()) break;
				await new Promise((r) => setTimeout(r, IDLE_POLL_INTERVAL_MS));
			}

			// Failed background task: deliver the error instead of going silent
			if (result.error) {
				ctx.ui.setStatus(
					"orchestrator",
					`Task ${result.taskId.slice(0, 8)}... failed ✗`,
				);
				pi.sendMessage(
					{
						customType: CUSTOM_MESSAGE_TYPE,
						content: [{ type: "text", text: `## Task Failed\n\n${result.error}` }],
						display: `orchestrator: task failed (${result.taskId.slice(0, 8)}...)`,
						details: {
							taskId: result.taskId,
							llamaModel: result.llamaConfig.model,
							llamaUrl: result.llamaConfig.url,
						},
					},
					{ deliverAs: "followUp", triggerTurn: true },
				);
				return;
			}

			ctx.ui.setStatus("orchestrator", "Done ✓");

			pi.sendMessage(
				{
					customType: CUSTOM_MESSAGE_TYPE,
					content: [{ type: "text", text: result.response!.content }],
					display: `orchestrator: ${result.llamaConfig.model}`,
					details: {
						llamaUsage: result.response!.usage,
						llamaModel: result.llamaConfig.model,
						llamaUrl: result.llamaConfig.url,
						taskId: result.taskId,
					},
				},
				{ deliverAs: "followUp", triggerTurn: true },
			);
		};

		deliverResult().catch((err) => {
			console.error("[orchestrator] Error delivering async result:", err);
		});
	};

	// Register listener on session start (ctx is needed for isIdle check)
	pi.on("session_start", (_event, ctx) => {
		currentCtx = ctx;
	});

	// Listen for async results from the tool
	pi.events.on("orchestrator:result", handleAsyncResult);

	// ── Tool definition ──

	pi.registerTool({
		name: "orchestrate",
		label: "Orchestrate",
		description: [
			"Run a task on the REMOTE llama.cpp server and return its answer. The task is",
			"ALWAYS executed remotely — nothing runs locally.",
			"",
			"USE THIS TOOL ONLY when the user explicitly asks for orchestration (e.g. their",
			"message contains the words 'orchestrate' or 'pi-orch', or asks to run something on the remote",
			"server). For all other requests (code review, questions, edits, etc.) do NOT call",
			"this tool — handle them locally with the normal tools.",
			"",
			"Flow: task (+ attached files) → remote llama.cpp server → answer",
			"",
			"Config can be set via orchestrator.json or environment variables:",
			"  ORCHESTRATOR_LLAMA_URL, ORCHESTRATOR_LLAMA_MODEL,",
			"  ORCHESTRATOR_LLAMA_SYSTEM_PROMPT, ORCHESTRATOR_LLAMA_MAX_TOKENS",
			"",
			"Sync (default): waits for the remote answer and returns it as the tool result.",
			"Async (async: true): fires the request in the background and returns immediately;",
			"the remote answer is delivered to the session when it arrives (waits for the",
			"session to be idle first).",
			"",
				"Use `sendFiles: [...]` to append the full contents of specific files to the",
				"prompt, so the remote model can review actual code (it has no file access).",
				"If the task mentions specific files (e.g. 'review src/foo.ts'), include those",
				"paths in `sendFiles` so the remote model can see their contents.",
				"",
				"For whole-project reviews use `sendDir` (default: cwd): the directory is tarred",
				"(excluding .git/node_modules/target/...), uploaded to the remote server's transfer",
				"API, and its review agent runs over the files — no prompt-size limit. Requires",
				"llamaApiKey (config or param).",
			].join("\n"),
		parameters: OrchestratorParams,

		async execute(_toolCallId, params, signal, _onUpdate, ctx) {
			const llamaConfig: LlamaCppConfig = {
				url: params.llamaUrl ?? config.llamaUrl,
				model: params.llamaModel ?? config.llamaModel,
				systemPrompt: params.llamaSystemPrompt ?? config.llamaSystemPrompt,
				maxTokens: params.llamaMaxTokens ?? config.llamaMaxTokens,
				temperature: params.llamaTemperature ?? config.llamaTemperature,
				topP: params.llamaTopP ?? config.llamaTopP,
				maxPromptLength: config.maxPromptLength,
				maxFileContent: config.maxFileContent,
			};

			// Pre-flight health check on the first call to a URL: a dead server
			// fails fast with a clear message instead of a 300s hang.
			if (lastPingedUrl !== llamaConfig.url) {
				const ping = await pingServer(llamaConfig.url);
				if (!ping.ok) {
					const msg =
						`llama.cpp server unreachable at ${llamaConfig.url} (${ping.error}). ` +
						`Is it running? Re-check with /orchestrator ping.`;
					return {
						content: [{ type: "text", text: msg }],
						isError: true,
						details: { llamaUrl: llamaConfig.url },
					};
				}
				lastPingedUrl = llamaConfig.url;
			}

			// Git diff attachment (optional): the most natural context for "review my changes"
			let diffSection: string | undefined;
			if (params.sendDiff) {
				const mode: DiffMode =
					params.sendDiff === true ? "unstaged" : params.sendDiff;
				diffSection = await buildDiffSection(
					params.cwd ?? ctx.cwd,
					mode,
					config.diffMaxSize,
				);
			}

			// ── Transfer mode (sendDir): tar + upload + remote review agent ──
			// The remote model reads the files itself, so there is no prompt-size limit.
			// Small directories that fit within maxPromptLength are inlined directly
			// (same path as sendFiles) to avoid unnecessary network round-trips.
			const isTransfer = params.sendDir !== undefined;
			let remoteCall: (sig: AbortSignal | undefined) => Promise<LlamaResponse>;

			if (isTransfer) {
				const apiKey = params.llamaApiKey ?? config.llamaApiKey;
				if (!apiKey) {
					return {
						content: [
							{
								type: "text",
								text:
									"sendDir (transfer mode) requires an API key: set llamaApiKey in orchestrator.json or ORCHESTRATOR_LLAMA_API_KEY.",
							},
						],
						isError: true,
					};
				}
				const dir = safeResolve(
					params.cwd ?? ctx.cwd,
					params.sendDir!,
				);
				if (!fs.existsSync(dir) || !fs.statSync(dir).isDirectory()) {
					return {
						content: [
							{ type: "text", text: `sendDir not found or not a directory: ${dir}` },
						],
						isError: true,
					};
				}
				// Estimate total size (excluding TAR_EXCLUDES). If it fits within
				// maxPromptLength, inline directly — no tarball needed.
				const dirSize = estimateDirSize(dir);
				if (dirSize <= llamaConfig.maxPromptLength) {
					// Small enough: inline the files directly (same as sendFiles)
					const files = dirToFiles(dir);
					const prompt = buildPrompt(
						params.task,
						files,
						dir,
						llamaConfig.maxFileContent,
						llamaConfig.maxPromptLength,
						diffSection,
					);
					remoteCall = (sig) => chatCompletion(llamaConfig, prompt, sig);
				} else {
					// Too large: use tarball + remote agent
					const task = diffSection
						? `${params.task}\n\n${diffSection}`
						: params.task;
					remoteCall = async (sig) => {
						ctx.ui.setStatus("orchestrator", `Tarring ${params.sendDir}...`);
						const tarball = await buildTarball(dir);
						if (tarball.length > MAX_TARBALL_SIZE) {
							throw new Error(
								`tarball is ${formatKb(tarball.length)} — over the ${formatKb(MAX_TARBALL_SIZE)} cap (use a narrower sendDir)`,
							);
						}
						ctx.ui.setStatus(
							"orchestrator",
							`Uploading ${formatKb(tarball.length)}...`,
						);
						const up = await uploadTransfer(
							llamaConfig.url,
							apiKey,
							tarball,
							"orchestrate",
							sig,
						);
						ctx.ui.setStatus("orchestrator", "Running remote agent...");
						const ag = await runAgent(llamaConfig.url, apiKey, up.id, task, sig);
						return {
							content: ag.answer,
							transferId: up.id,
							rounds: ag.rounds,
							filesRead: ag.files_read,
						};
					};
				}
			} else {
				// Inline mode: task + diff + attached file contents in the prompt
				const prompt = buildPrompt(
					params.task,
					params.sendFiles,
					params.cwd ?? ctx.cwd,
					llamaConfig.maxFileContent,
					llamaConfig.maxPromptLength,
					diffSection,
				);
				remoteCall = (sig) => chatCompletion(llamaConfig, prompt, sig);
			}

			// Async mode: fire in background, return immediately
			if (params.async) {
				const taskId = crypto.randomUUID();
				ctx.ui.setStatus("orchestrator", `Task queued (${taskId.slice(0, 8)}...)`);

				// Track for `/orchestrator status` (cap the registry)
				const entry: TrackedTask = {
					taskId,
					task: params.task,
					model: llamaConfig.model,
					startedAt: Date.now(),
					state: "running",
				};
				tasks.set(taskId, entry);
				if (tasks.size > MAX_TRACKED_TASKS) {
					const oldest = tasks.keys().next().value;
					if (oldest) {
						tasks.delete(oldest);
						abortControllers.get(oldest)?.abort();
						abortControllers.delete(oldest);
					}
				}

				// Fire the remote request in the background, emit result on completion.
				// NOTE: a fresh AbortController — NOT the tool's AbortSignal, which
				// aborts immediately when the tool returns. Our controller lets
				// `/orchestrator kill` cancel the task on demand.
				const controller = new AbortController();
				abortControllers.set(taskId, controller);
				remoteCall(controller.signal)
					.then((response) => {
						if (entry.state === "running") entry.state = "done";
						entry.finishedAt = Date.now();
						abortControllers.delete(taskId);
						pi.events.emit("orchestrator:result", {
							taskId,
							response,
							llamaConfig,
						});
					})
					.catch((err) => {
						const errorContent = controller.signal.aborted
							? "Task killed by user"
							: `llama.cpp request failed: ${(err as Error).message}`;
						if (entry.state === "running") {
							entry.state = "failed";
							entry.error = errorContent;
						}
						entry.finishedAt = Date.now();
						abortControllers.delete(taskId);
						console.error(`[orchestrator] Remote request failed for ${taskId}:`, err);
						// Redact apiKey before emitting (P1: prevent key exposure in logs/events)
						const safeConfig = { ...llamaConfig, llamaApiKey: "" };
						pi.events.emit("orchestrator:result", {
							taskId,
							error: errorContent,
							llamaConfig: safeConfig,
						});
					});

				return {
					content: [
						{
							type: "text",
							text: `Task queued (id: ${taskId.slice(0, 8)}...). Will notify when done.`,
						},
					],
					isError: false,
				};
			}

			// ── Sync mode: wait for the remote answer and return it ──
			ctx.ui.setStatus(
				"orchestrator",
				isTransfer ? "Starting transfer..." : "Sending to remote llama.cpp...",
			);

			try {
				const response = await remoteCall(signal);

				ctx.ui.setStatus("orchestrator", "Done ✓");

				return {
					content: [{ type: "text", text: response.content }],
					details: {
						llamaUsage: response.usage,
						llamaModel: llamaConfig.model,
						llamaUrl: llamaConfig.url,
						transferId: response.transferId,
						rounds: response.rounds,
						filesRead: response.filesRead,
					},
				};
			} catch (err) {
				return {
					content: [
						{
							type: "text",
							text: isTransfer
								? `transfer/agent run failed: ${(err as Error).message}`
								: `llama.cpp request failed: ${(err as Error).message}`,
						},
					],
					isError: true,
					details: {
						llamaUrl: llamaConfig.url,
						llamaModel: llamaConfig.model,
					},
				};
			}
		},

		renderCall(args, theme, _context) {
			const preview =
				args.task.length > 60 ? `${args.task.slice(0, 60)}...` : args.task;
			const model = args.llamaModel || config.llamaModel;
			const url = args.llamaUrl || config.llamaUrl;

			let text =
				theme.fg("toolTitle", theme.bold("orchestrate ")) +
				theme.fg("accent", model) +
				theme.fg("muted", ` @ ${url} (remote)`);
			if (args.sendDir) {
				text += theme.fg("accent", ` +dir:${args.sendDir}`);
			}
			if (args.sendDiff) {
				const mode = args.sendDiff === true ? "unstaged" : args.sendDiff;
				text += theme.fg("accent", ` +diff:${mode}`);
			}
			text += `\n  ${theme.fg("dim", preview)}`;
			return new Text(text, 0, 0);
		},

		renderResult(result, { expanded }, theme, _context) {
			const details = result.details as
				| {
						llamaUsage?: { input: number; output: number };
						llamaModel?: string;
						llamaUrl?: string;
						taskId?: string;
						transferId?: string;
						rounds?: number;
						filesRead?: string[];
				  }
				| undefined;

			if (result.isError) {
				const text = result.content[0];
				return new Text(
					`${theme.fg("error", "✗ ")}${text?.type === "text" ? text.text : "(error)"}`,
					0,
					0,
				);
			}

			// Async queued status
			if (details?.taskId) {
				return new Text(
					theme.fg(
						"muted",
						`⏳ Task queued (${details.taskId.slice(0, 8)}...) — result will be delivered when done`,
					),
					0,
					0,
				);
			}

			const text = result.content[0];
			const answer = text?.type === "text" ? text.text : "(no output)";

			if (expanded) {
				const container = new Container();
				container.addChild(
					new Text(
						theme.fg("toolTitle", theme.bold("orchestrate ")) +
							theme.fg("accent", details?.llamaModel || config.llamaModel) +
							theme.fg("muted", ` @ ${details?.llamaUrl || config.llamaUrl} (remote)`),
						0,
						0,
					),
				);

				if (details?.llamaUsage) {
					const llamaUsageStr = `↑${details.llamaUsage.input} ↓${details.llamaUsage.output}`;
					container.addChild(
						new Text(theme.fg("dim", `llama.cpp: ${llamaUsageStr}`), 0, 0),
					);
				}

				if (details?.transferId) {
					const files = details.filesRead?.length
						? `${details.filesRead.length} file(s) read`
						: "no files read";
					container.addChild(
						new Text(
							theme.fg("dim", `transfer ${details.transferId} · ${details.rounds ?? "?"} round(s) · ${files}`),
							0,
						0,
					),
					);
				}

				container.addChild(new Spacer(1));
				container.addChild(
					new Text(theme.fg("muted", "─── Remote Answer ───"), 0, 0),
				);
				container.addChild(new Markdown(answer, 0, 0, getMarkdownTheme()));

				return container;
			}

			// Collapsed: show just the answer
			const maxLines = 5;
			const lines = answer.split("\n");
			const preview = lines.slice(0, maxLines).join("\n");
			const more =
				lines.length > maxLines
					? `\n${theme.fg("muted", "(Ctrl+O to expand)")}`
					: "";

			return new Text(
				`${theme.fg("muted", "llama.cpp: ")}${preview}${more}`,
				0,
				0,
			);
		},
	});

	// ── Custom message renderer for async results ──
	pi.registerMessageRenderer(
		CUSTOM_MESSAGE_TYPE,
		(message, { expanded }, theme) => {
			const content = message.content[0];
			const finalText = content?.type === "text" ? content.text : "(no output)";

			// Failed background task: render as a plain error
			if (finalText.startsWith("## Task Failed")) {
				const errorText = finalText.replace(/^## Task Failed\n\n/, "");
				return new Text(theme.fg("error", `✗ orchestrator: ${errorText}`), 0, 0);
			}

			const details = message.details as
				| {
						llamaUsage?: { input: number; output: number };
						llamaModel?: string;
						llamaUrl?: string;
						taskId?: string;
				  }
				| undefined;

			if (expanded) {
				const container = new Container();
				container.addChild(
					new Text(
						theme.fg("toolTitle", theme.bold("orchestrator: ")) +
							theme.fg("accent", details?.llamaModel || config.llamaModel) +
							theme.fg("muted", ` @ ${details?.llamaUrl || config.llamaUrl} (remote)`),
						0,
						0,
					),
				);

				if (details?.llamaUsage) {
					const llamaUsageStr = `↑${details.llamaUsage.input} ↓${details.llamaUsage.output}`;
					container.addChild(
						new Text(theme.fg("dim", `llama.cpp: ${llamaUsageStr}`), 0, 0),
					);
				}

				container.addChild(new Spacer(1));
				container.addChild(
					new Text(theme.fg("muted", "─── Remote Answer ───"), 0, 0),
				);
				container.addChild(new Markdown(finalText, 0, 0, getMarkdownTheme()));

				return container;
			}

			const maxLines = 5;
			const lines = finalText.split("\n");
			const preview = lines.slice(0, maxLines).join("\n");
			const more =
				lines.length > maxLines
					? `\n${theme.fg("muted", "(Ctrl+O to expand)")}`
					: "";

			return new Text(
				`${theme.fg("muted", "orchestrator: ")}${preview}${more}`,
				0,
				0,
			);
		},
	);

	// ── /orchestrator status command ──
	pi.registerCommand("orchestrator", {
		description:
			"Orchestrator background tasks: /orchestrator status — list tasks, /orchestrator kill <id|all> — kill a running task (or all), /orchestrator ping [url] — health check",
		getArgumentCompletions: (prefix: string) => {
			const trimmed = prefix.trimStart();
			if (trimmed.startsWith("kill ")) {
				const idPrefix = trimmed.slice(5);
				const matches = [
					{id: "all", label: "all (kill all running)"},
					...[...tasks.values()]
						.filter(
							(t) =>
								t.state === "running" && t.taskId.startsWith(idPrefix),
						)
						.map((t) => ({
							id: t.taskId.slice(0, 8),
							label: t.taskId.slice(0, 8),
						})),
				].filter((m) => m.id.startsWith(idPrefix));
				return matches.length > 0
					? matches.map((m) => ({
							value: `kill ${m.id}`,
							label: m.label,
						}))
					: null;
			}
			if (trimmed.startsWith("ping")) return null;
			const items = ["status", "kill", "ping"].filter((c) =>
				c.startsWith(trimmed),
			);
			return items.length > 0 ? items.map((c) => ({ value: c, label: c })) : null;
		},
			handler: async (args, ctx) => {
			const [cmd, idArg] = (args || "").trim().split(/\s+/);
			if (cmd === "ping") {
				const url = idArg || config.llamaUrl;
				ctx.ui.setStatus("orchestrator", `Pinging ${url}...`);
				const result = await pingServer(url);
				if (result.ok) {
					ctx.ui.setStatus("orchestrator", `ok (${result.latencyMs}ms)`);
					ctx.ui.notify(
						`✓ ${url} ok (${result.latencyMs}ms)`,
						"info",
					);
				} else {
					ctx.ui.setStatus("orchestrator", "unreachable");
					ctx.ui.notify(
						`✗ ${url} unreachable: ${result.error}`,
						"error",
					);
				}
				return;
			}
			if (cmd === "kill") {
				if (!idArg) {
					ctx.ui.notify(
						"Usage: /orchestrator kill <task-id-prefix|all>",
						"warning",
					);
					return;
				}
				if (idArg === "all") {
					const running = [...tasks.values()].filter(
						(t) => t.state === "running",
					);
					if (running.length === 0) {
						ctx.ui.notify("No running tasks to kill", "info");
						return;
					}
					for (const t of running) {
						abortControllers.get(t.taskId)?.abort();
						abortControllers.delete(t.taskId); // prevent memory leak
						t.state = "killed";
						t.error = "Killed by user";
						t.finishedAt = Date.now();
					}
					ctx.ui.setStatus(
						"orchestrator",
						`Killed ${running.length} task(s)`,
					);
					ctx.ui.notify(
						`Killed ${running.length} task(s)`,
						"info",
					);
					return;
				}
				const match = [...tasks.values()].find(
					(t) => t.state === "running" && t.taskId.startsWith(idArg),
				);
				if (!match) {
					ctx.ui.notify(`No running task matching "${idArg}"`, "warning");
					return;
				}
				abortControllers.get(match.taskId)?.abort();
				abortControllers.delete(match.taskId); // prevent memory leak
				match.state = "killed";
				match.error = "Killed by user";
				match.finishedAt = Date.now();
				ctx.ui.setStatus(
					"orchestrator",
					`Task ${match.taskId.slice(0, 8)}... killed`,
				);
				ctx.ui.notify(`Killed task ${match.taskId.slice(0, 8)}…`, "info");
				return;
			}
			if (cmd !== "status") {
				ctx.ui.notify(
					"Usage: /orchestrator status | /orchestrator kill <id|all> | /orchestrator ping [url]",
					"warning",
				);
				return;
			}
			const list = [...tasks.values()];
			if (list.length === 0) {
				ctx.ui.notify("orchestrator: no background tasks", "info");
				return;
			}
			const running = list.filter((t) => t.state === "running").length;
			// TUI-only entry — does not participate in LLM context
			pi.appendEntry(STATUS_ENTRY_TYPE, {
				running,
				tasks: list.map((t) => ({ ...t })),
			});
		},
	});

	// ── Entry renderer for the task list ──
	pi.registerEntryRenderer(STATUS_ENTRY_TYPE, (entry, _opts, theme) => {
		const data = entry.data as { running: number; tasks: TrackedTask[] };
		const container = new Container();
		container.addChild(
			new Text(
				theme.fg("toolTitle", theme.bold("orchestrator: ")) +
					theme.fg("muted", `${data.running} running, ${data.tasks.length} total`),
				0,
				0,
			),
		);
		for (const t of data.tasks) {
			const icon = { running: "⏳", done: "✓", failed: "✗", killed: "⊘" }[t.state];
			const color = {
				running: "accent",
				done: "dim",
				failed: "error",
				killed: "muted",
			}[t.state];
			const preview = t.task.length > 60 ? `${t.task.slice(0, 60)}…` : t.task;
			const end = t.finishedAt ?? Date.now();
			const dur = `${Math.max(1, Math.round((end - t.startedAt) / 1000))}s${t.finishedAt ? "" : "+"}`;
			container.addChild(
				new Text(
					theme.fg(
						color,
						`${icon} ${t.taskId.slice(0, 8)}  ${dur.padStart(7)}  ${t.model}  ${preview}`,
					),
					0,
					0,
				),
			);
			if (t.error) {
				container.addChild(
					new Text(theme.fg("error", `    ${t.error.split("\n")[0]}`), 0, 0),
				);
			}
		}
		return container;
	});
}
