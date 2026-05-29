#!/usr/bin/env node
// MCP server that wraps the `gfit-cli` binary so Claude Desktop can drive the GFIT API.
// Reserves stdout for JSON-RPC; all logging goes to stderr.

import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  ListToolsRequestSchema,
  CallToolRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const HOME = homedir();
// macOS GUI apps (Claude Desktop) often launch with a minimal PATH that omits
// /usr/local/bin etc., so resolve common install locations explicitly.
const EXTRA_BINDIRS = [
  "/usr/local/bin",
  "/opt/homebrew/bin",
  join(HOME, ".cargo", "bin"),
  join(HOME, ".local", "bin"),
];

function resolveBinary() {
  const configured = (process.env.GFIT_CLI_PATH || "").trim();
  if (configured && configured !== "gfit-cli") return configured; // explicit path
  for (const dir of EXTRA_BINDIRS) {
    const cand = join(dir, "gfit-cli");
    if (existsSync(cand)) return cand;
  }
  return "gfit-cli"; // fall back to a PATH lookup
}

function childEnv() {
  const env = { ...process.env };
  const extra = EXTRA_BINDIRS.join(":");
  env.PATH = env.PATH ? `${env.PATH}:${extra}` : extra;
  return env;
}

function runGfit(args, { timeoutMs = 60000 } = {}) {
  return new Promise((resolve) => {
    const bin = resolveBinary();
    let stdout = "";
    let stderr = "";
    let done = false;
    let child;
    try {
      child = spawn(bin, args, { env: childEnv() });
    } catch (e) {
      resolve({ code: -1, stdout: "", stderr: `failed to spawn ${bin}: ${e.message}` });
      return;
    }
    const finish = (r) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      resolve(r);
    };
    const timer = setTimeout(() => {
      try { child.kill("SIGKILL"); } catch {}
      finish({ code: -1, stdout, stderr: `${stderr}\n[timed out after ${timeoutMs}ms]` });
    }, timeoutMs);
    child.stdout.on("data", (d) => (stdout += d.toString()));
    child.stderr.on("data", (d) => (stderr += d.toString()));
    child.on("error", (e) =>
      finish({
        code: -1,
        stdout,
        stderr: `failed to run ${bin}: ${e.message}. Is gfit-cli installed? Set its path in the extension's configuration.`,
      })
    );
    child.on("close", (code) => finish({ code: code ?? -1, stdout, stderr }));
  });
}

function flattenParams(params) {
  const args = [];
  for (const [k, v] of Object.entries(params || {})) {
    args.push(`--${k}`);
    if (v === true || v === null || v === undefined) continue; // bare flag
    args.push(typeof v === "object" ? JSON.stringify(v) : String(v));
  }
  return args;
}

// Verbs that mutate production data; these get a dry-run preview unless confirmed.
const WRITE_RE =
  /(create|update|delete|generate|publish|duplicate|attach|detach|notice|merge|upload|change|set-primary|reindex|answer-update|log-update|log-delete)/i;
const isWriteCommand = (cmd) => WRITE_RE.test(cmd);

const textResult = (text, isError = false) => ({
  content: [{ type: "text", text }],
  isError,
});

const TOOLS = [
  {
    name: "gfit_list_commands",
    description:
      "List every gfit-cli command, grouped (auth, admin, coach, plan, self) — the full ~144-command surface. Use this to discover what's available before calling gfit_run.",
    inputSchema: { type: "object", properties: {}, additionalProperties: false },
  },
  {
    name: "gfit_help",
    description:
      "Show one command's API docs: endpoint, whether auth is required, and every parameter with type and meaning. Check this before calling a command you haven't used.",
    inputSchema: {
      type: "object",
      properties: {
        command: { type: "string", description: "Dot-named command, e.g. coach.checkin.list" },
      },
      required: ["command"],
      additionalProperties: false,
    },
  },
  {
    name: "gfit_auth_status",
    description:
      "Show whether gfit-cli is logged in (account email, token presence, config path, API URL). If not logged in, the user must run `gfit-cli auth.login` once in a terminal.",
    inputSchema: { type: "object", properties: {}, additionalProperties: false },
  },
  {
    name: "gfit_run",
    description:
      "Run any gfit-cli command against the GFIT API. Provide the dot-named `command` and a `params` object (each key becomes --key value; arbitrary keys are allowed — gfit-cli auto-types them). Reads (list/info/search/*.list) run directly. Write and destructive commands (create/update/delete/generate/publish/notice/attach/detach/…) return a --dry-run PREVIEW unless you pass confirm:true — note *.notice sends client emails and *.delete removes data, so review the preview first. Output is the API response JSON (success = code 1, payload under data).",
    inputSchema: {
      type: "object",
      properties: {
        command: { type: "string", description: "Dot-named command, e.g. coach.clients" },
        params: {
          type: "object",
          description: 'Request body params as key/value, e.g. {"id": 42}. Arbitrary keys allowed.',
          additionalProperties: true,
        },
        dry_run: {
          type: "boolean",
          description: "Force a request preview without sending it (needs no token).",
        },
        raw: { type: "boolean", description: "Return compact JSON (default true)." },
        confirm: {
          type: "boolean",
          description:
            "Required to actually execute a write/destructive command instead of getting a dry-run preview.",
        },
      },
      required: ["command"],
      additionalProperties: false,
    },
  },
];

const server = new Server(
  { name: "gfit-cli", version: "0.1.0" },
  { capabilities: { tools: {} } }
);

server.setRequestHandler(ListToolsRequestSchema, async () => ({ tools: TOOLS }));

server.setRequestHandler(CallToolRequestSchema, async (req) => {
  const { name, arguments: a = {} } = req.params;
  try {
    if (name === "gfit_list_commands") {
      const r = await runGfit([]);
      return textResult(r.stdout || r.stderr || "(no output)");
    }
    if (name === "gfit_help") {
      const command = String(a.command || "").trim();
      if (!command) return textResult("command is required", true);
      const r = await runGfit([command, "-h"]);
      return textResult(r.stdout || r.stderr || "(no output)", r.code !== 0);
    }
    if (name === "gfit_auth_status") {
      const r = await runGfit(["auth.status"]);
      return textResult(r.stdout || r.stderr || "(no output)", r.code !== 0);
    }
    if (name === "gfit_run") {
      const command = String(a.command || "").trim();
      if (!command) return textResult("command is required", true);
      if (command === "auth.login") {
        return textResult(
          "auth.login uses an interactive browser sign-in and isn't available as a tool. Run `gfit-cli auth.login` once in a terminal; the saved token is then used here automatically.",
          true
        );
      }
      const raw = a.raw !== false; // default true
      const wantsWrite = isWriteCommand(command);
      const forcedPreview = wantsWrite && a.confirm !== true && a.dry_run !== true;
      const preview = a.dry_run === true || forcedPreview;

      const args = [command, ...flattenParams(a.params)];
      if (raw) args.push("--raw");
      if (preview) args.push("--dry-run");

      const r = await runGfit(args);
      const note = forcedPreview
        ? `⚠️ "${command}" is a write/destructive command, so this is a --dry-run PREVIEW (nothing was sent). Review the request below; to execute it for real, call gfit_run again with confirm:true.\n\n`
        : "";
      const body = r.stdout || r.stderr || "(no output)";
      // A forced/explicit preview that ran fine is not an error even on a write path.
      const isError = r.code !== 0 && !preview;
      return textResult(note + body, isError);
    }
    return textResult(`unknown tool: ${name}`, true);
  } catch (e) {
    return textResult(`error: ${e.message}`, true);
  }
});

const transport = new StdioServerTransport();
await server.connect(transport);
console.error("gfit-cli MCP server running on stdio");
