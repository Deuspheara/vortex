/**
 * Minimal JSON-RPC sidecar for virtual bash execution via just-bash OverlayFs.
 * Protocol: newline-delimited JSON on stdin/stdout.
 */

import { Bash, OverlayFs } from "just-bash";

type JsonRpcRequest = {
  id: string;
  method: string;
  params: Record<string, unknown>;
};

type JsonRpcResponse = {
  id: string;
  result?: unknown;
  error?: { message: string };
};

type BashOutputNotification = {
  method: "bash.output";
  params: {
    request_id: string;
    stream: "stdout" | "stderr";
    chunk: string;
  };
};

type BashProgressNotification = {
  method: "bash.progress";
  params: {
    request_id: string;
    elapsed_ms: number;
  };
};

const CHUNK_SIZE = 4096;
const PROGRESS_INTERVAL_MS = 2000;

function emitChunks(requestId: string, stream: "stdout" | "stderr", text: string) {
  for (let i = 0; i < text.length; i += CHUNK_SIZE) {
    const chunk = text.slice(i, i + CHUNK_SIZE);
    const notification: BashOutputNotification = {
      method: "bash.output",
      params: { request_id: requestId, stream, chunk },
    };
    process.stdout.write(`${JSON.stringify(notification)}\n`);
  }
}

function emitProgress(requestId: string, elapsedMs: number) {
  const notification: BashProgressNotification = {
    method: "bash.progress",
    params: { request_id: requestId, elapsed_ms: elapsedMs },
  };
  process.stdout.write(`${JSON.stringify(notification)}\n`);
}

async function handleExec(params: Record<string, unknown>, requestId: string) {
  const script = String(params.script ?? "");
  const projectRoot = String(params.project_root ?? process.cwd());
  const timeoutMs = Number(params.timeout_ms ?? 30000);

  emitProgress(requestId, 0);

  try {
    const overlay = new OverlayFs({ root: projectRoot });
    const bash = new Bash({
      fs: overlay,
      cwd: overlay.getMountPoint(),
      network: undefined,
      javascript: false,
      python: false,
      executionLimits: {
        maxCallDepth: 50,
        maxCommandCount: Number(params.max_command_count ?? 1000),
        maxLoopIterations: 1000,
        maxAwkIterations: 1000,
        maxSedIterations: 1000,
      },
    });

    const controller = new AbortController();
    const timer = setTimeout(() => {
      controller.abort();
      emitChunks(requestId, "stderr", `Command timed out after ${timeoutMs}ms\n`);
    }, timeoutMs);
    const started = Date.now();
    const progressTimer = setInterval(() => {
      emitProgress(requestId, Date.now() - started);
    }, PROGRESS_INTERVAL_MS);

    const result = await bash.exec(script, { signal: controller.signal });
    clearInterval(progressTimer);
    clearTimeout(timer);

    const stdout = result.stdout ?? "";
    const stderr = result.stderr ?? "";
    emitChunks(requestId, "stdout", stdout);
    emitChunks(requestId, "stderr", stderr);

    return {
      stdout,
      stderr,
      exit_code: result.exitCode ?? 0,
      changed_files: [],
      virtual_diff: "",
      observed_commands: result.metadata?.commands ?? [],
      duration_ms: Date.now() - started,
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    emitChunks(requestId, "stderr", message);
    return {
      stdout: "",
      stderr: message,
      exit_code: 1,
      changed_files: [],
      virtual_diff: "",
      observed_commands: [],
      duration_ms: 0,
    };
  }
}

async function handleRequest(req: JsonRpcRequest): Promise<JsonRpcResponse> {
  try {
    switch (req.method) {
      case "ping":
        return { id: req.id, result: { ok: true } };
      case "bash.exec":
        return { id: req.id, result: await handleExec(req.params, req.id) };
      default:
        return { id: req.id, error: { message: `unknown method: ${req.method}` } };
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { id: req.id, error: { message } };
  }
}

const decoder = new TextDecoder();
let buffer = "";

process.stdin.on("data", async (chunk: Buffer) => {
  buffer += decoder.decode(chunk, { stream: true });
  let newlineIndex = buffer.indexOf("\n");
  while (newlineIndex !== -1) {
    const line = buffer.slice(0, newlineIndex).trim();
    buffer = buffer.slice(newlineIndex + 1);
    if (line.length > 0) {
      const req = JSON.parse(line) as JsonRpcRequest;
      const resp = await handleRequest(req);
      process.stdout.write(`${JSON.stringify(resp)}\n`);
    }
    newlineIndex = buffer.indexOf("\n");
  }
});
