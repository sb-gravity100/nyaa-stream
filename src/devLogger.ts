import { invoke } from "@tauri-apps/api/core";
import { isTauriAvailable } from "./browserFallback";

// The native Tauri window has no accessible devtools console during
// development, so frontend errors are otherwise invisible outside the app
// itself. This forwards console output and uncaught errors/rejections into
// the backend's `tracing` log (see src-tauri's `log_frontend` command),
// which lands in the same terminal output as every Rust-side log line.
// No-ops in the browser-only dev preview, which already has real devtools.

function stringifyArg(arg: unknown): string {
  if (typeof arg === "string") return arg;
  if (arg instanceof Error) return `${arg.name}: ${arg.message}\n${arg.stack ?? ""}`;
  try {
    return JSON.stringify(arg);
  } catch {
    return String(arg);
  }
}

function forward(level: "debug" | "info" | "warn" | "error", args: unknown[]): void {
  const message = args.map(stringifyArg).join(" ");
  void invoke("log_frontend", { level, message }).catch(() => {
    // Backend unreachable (e.g. shutting down) - nothing more to do.
  });
}

export function installDevLogger(): void {
  if (!isTauriAvailable()) return;

  const original = {
    debug: console.debug.bind(console),
    log: console.log.bind(console),
    info: console.info.bind(console),
    warn: console.warn.bind(console),
    error: console.error.bind(console),
  };

  console.debug = (...args: unknown[]) => {
    original.debug(...args);
    forward("debug", args);
  };
  console.log = (...args: unknown[]) => {
    original.log(...args);
    forward("debug", args);
  };
  console.info = (...args: unknown[]) => {
    original.info(...args);
    forward("info", args);
  };
  console.warn = (...args: unknown[]) => {
    original.warn(...args);
    forward("warn", args);
  };
  console.error = (...args: unknown[]) => {
    original.error(...args);
    forward("error", args);
  };

  window.addEventListener("error", (event) => {
    forward("error", [`Uncaught error: ${event.message}`, event.error]);
  });
  window.addEventListener("unhandledrejection", (event) => {
    forward("error", ["Unhandled promise rejection:", event.reason]);
  });

  console.info("[devLogger] installed - frontend console/errors now forwarded to the Rust backend log");
}
