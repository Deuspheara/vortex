/**
 * Minimal JSON-RPC sidecar for browser read-only inspection.
 * Protocol: newline-delimited JSON on stdin/stdout.
 *
 * Exposes only browser.snapshot and browser.screenshot. It intentionally has no
 * click/type/submit APIs.
 */

import { chromium, type Browser, type Page } from "playwright";

type JsonRpcRequest = {
  id: string;
  method: string;
  params: {
    timeout_ms?: number;
    params?: Record<string, unknown>;
  };
};

type JsonRpcResponse = {
  id: string;
  result?: unknown;
  error?: { message: string };
};

type BoundingBox = {
  x: number;
  y: number;
  width: number;
  height: number;
};

const DEFAULT_TIMEOUT_MS = 30000;
const MAX_TEXT_CHARS = 50000;
const MAX_DOM_CHARS = 50000;

let browserPromise: Promise<Browser> | undefined;

function getBrowser(): Promise<Browser> {
  if (!browserPromise) {
    browserPromise = chromium.launch({
      headless: true,
      chromiumSandbox: true,
    });
  }
  return browserPromise;
}

function assertPublicHttpUrl(raw: unknown): string {
  const url = String(raw ?? "");
  const parsed = new URL(url);
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
    throw new Error("only absolute http(s) URLs are allowed");
  }
  const host = parsed.hostname.toLowerCase();
  if (
    host === "localhost" ||
    host.endsWith(".localhost") ||
    host.endsWith(".local") ||
    host.endsWith(".internal") ||
    host === "127.0.0.1" ||
    host === "::1" ||
    host.startsWith("10.") ||
    host.startsWith("192.168.") ||
    /^172\.(1[6-9]|2\d|3[0-1])\./.test(host)
  ) {
    throw new Error("local, private, and internal network hosts are not allowed");
  }
  return parsed.toString();
}

async function newPage(timeoutMs: number): Promise<Page> {
  const browser = await getBrowser();
  const context = await browser.newContext({
    acceptDownloads: false,
    ignoreHTTPSErrors: false,
    bypassCSP: false,
    javaScriptEnabled: true,
  });
  context.setDefaultTimeout(timeoutMs);
  context.setDefaultNavigationTimeout(timeoutMs);
  return context.newPage();
}

async function waitForPage(page: Page, waitFor: unknown, selector: unknown) {
  const mode = String(waitFor ?? "load");
  if (mode === "selector") {
    if (!selector) {
      throw new Error("selector is required when wait_for=selector");
    }
    await page.waitForSelector(String(selector), { state: "visible" });
  } else if (mode === "networkidle") {
    await page.waitForLoadState("networkidle");
  } else {
    await page.waitForLoadState("load");
  }
}

async function interactiveElements(page: Page) {
  return page.locator("a,button,input,textarea,select,[role=button],[role=link]").evaluateAll(
    (nodes) =>
      nodes.slice(0, 100).map((node) => {
        const rect = (node as HTMLElement).getBoundingClientRect();
        const element = node as HTMLElement;
        return {
          role: element.getAttribute("role") ?? element.tagName.toLowerCase(),
          name:
            element.getAttribute("aria-label") ??
            element.getAttribute("title") ??
            element.innerText?.trim() ??
            "",
          selector: element.id ? `#${element.id}` : element.tagName.toLowerCase(),
          bbox: {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
          } satisfies BoundingBox,
        };
      }),
  );
}

async function handleSnapshot(params: Record<string, unknown>, timeoutMs: number) {
  const url = assertPublicHttpUrl(params.url);
  const page = await newPage(timeoutMs);
  try {
    await page.goto(url, { waitUntil: "domcontentloaded", timeout: timeoutMs });
    await waitForPage(page, params.wait_for, params.selector);
    const title = await page.title();
    const visibleText = (await page.locator("body").innerText({ timeout: 5000 })).slice(
      0,
      MAX_TEXT_CHARS,
    );
    const accessibilityTree =
      params.include_accessibility_tree === false
        ? undefined
        : await page.accessibility.snapshot({ interestingOnly: true });
    const domSummary =
      params.include_dom === true
        ? (await page.locator("body").evaluate((body) => body.outerHTML)).slice(0, MAX_DOM_CHARS)
        : undefined;
    return {
      url: page.url(),
      title,
      visibleText,
      accessibilityTree,
      domSummary,
      interactiveElements: await interactiveElements(page),
      confidence: "high",
      warnings: [],
      provenance: {
        source_url: url,
        final_url: page.url(),
        provider: "playwright",
        fetched_at: new Date().toISOString(),
        extraction_mode: "browser_snapshot",
      },
      untrusted: true,
    };
  } finally {
    await page.context().close();
  }
}

async function handleScreenshot(params: Record<string, unknown>, timeoutMs: number) {
  const url = assertPublicHttpUrl(params.url);
  const outputPath = String(params.output_path ?? "");
  if (!outputPath) {
    throw new Error("output_path is required");
  }
  const page = await newPage(timeoutMs);
  try {
    await page.goto(url, { waitUntil: "domcontentloaded", timeout: timeoutMs });
    await waitForPage(page, params.wait_for, params.selector);
    const selector = params.selector ? String(params.selector) : undefined;
    if (selector) {
      await page.locator(selector).first().screenshot({ path: outputPath });
    } else {
      await page.screenshot({
        path: outputPath,
        fullPage: params.full_page !== false,
      });
    }
    return {
      url: page.url(),
      title: await page.title(),
      image_path: outputPath,
      selector,
      full_page: selector ? false : params.full_page !== false,
      confidence: "high",
      warnings: [],
      provenance: {
        source_url: url,
        final_url: page.url(),
        provider: "playwright",
        fetched_at: new Date().toISOString(),
        extraction_mode: "browser_screenshot",
      },
      untrusted: true,
    };
  } finally {
    await page.context().close();
  }
}

async function handleRequest(req: JsonRpcRequest): Promise<JsonRpcResponse> {
  try {
    const timeoutMs = Number(req.params.timeout_ms ?? DEFAULT_TIMEOUT_MS);
    const params = req.params.params ?? {};
    switch (req.method) {
      case "ping":
        return { id: req.id, result: { ok: true } };
      case "browser.snapshot":
        return { id: req.id, result: await handleSnapshot(params, timeoutMs) };
      case "browser.screenshot":
        return { id: req.id, result: await handleScreenshot(params, timeoutMs) };
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
