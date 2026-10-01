import { useState } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";

// Settings -> Help (PLAN.md "Contact and send logs"). Nothing is uploaded:
// Send logs writes a zip to Downloads and the user sends it by hand.

const REPO_URL = "https://github.com/sb-gravity100/nyaa-stream";
/** The author's Discord username - shown with a copy button (a profile
 * link would need the numeric user id). */
const DISCORD_USERNAME = "__sb______";

function open(url: string) {
  console.info("[help] opening link", { url });
  openUrl(url).catch((err) => console.warn("[help] couldn't open link", { url, err: String(err) }));
}

/** "Discord: <name>" with a button that copies the name. */
function DiscordName({ hint }: { hint: string }) {
  const [copied, setCopied] = useState(false);
  async function copy() {
    console.info("[help] copying the Discord username");
    try {
      await navigator.clipboard.writeText(DISCORD_USERNAME);
      setCopied(true);
    } catch (err) {
      console.warn("[help] clipboard write failed", { err: String(err) });
    }
  }
  return (
    <div class="help-discord">
      <span>
        Discord: <strong>{DISCORD_USERNAME}</strong> <span class="setting-hint">{hint}</span>
      </span>
      <button class="button button-quiet" onClick={() => void copy()}>
        {copied ? "Copied" : "Copy"}
      </button>
    </div>
  );
}

function Modal({ title, onClose, children }: { title: string; onClose: () => void; children: preact.ComponentChildren }) {
  return (
    <div
      class="export-backdrop help-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        class="export-dialog help-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onKeyDown={(e) => {
          if (e.key === "Escape") onClose();
        }}
      >
        <h2 class="export-title">{title}</h2>
        {children}
        <div class="export-actions">
          <button class="button button-quiet" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}

function ContactModal({ onClose }: { onClose: () => void }) {
  return (
    <Modal title="Contact" onClose={onClose}>
      <div class="help-links">
        <button class="button button-quiet" onClick={() => open(REPO_URL)}>
          GitHub repository
        </button>
      </div>
      <DiscordName hint="add me as a friend to message" />
    </Modal>
  );
}

function SendLogsModal({ onClose }: { onClose: () => void }) {
  const [path, setPath] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function exportLogs() {
    console.info("[help] export logs pressed");
    setBusy(true);
    setMessage(null);
    try {
      const saved = await invoke<string>("export_logs");
      setPath(saved);
      revealItemInDir(saved).catch((err) => console.warn("[help] reveal failed", { err: String(err) }));
    } catch (err) {
      setMessage(String(err));
    }
    setBusy(false);
  }

  async function openIssue() {
    const version = await getVersion().catch(() => "unknown");
    const body = `**App version:** ${version}\n**OS:** Windows\n\n**What happened?**\n\n\n**Logs:** attach the nyaa-stream-logs zip from your Downloads folder.\n`;
    open(`${REPO_URL}/issues/new?title=${encodeURIComponent("Bug: ")}&body=${encodeURIComponent(body)}`);
  }

  return (
    <Modal title="Send logs" onClose={onClose}>
      <p class="setting-hint">
        Saves a zip of the last week's logs and basic system info to your Downloads folder. Your Windows user name is removed; the titles
        you watched are included. GitHub issues are public.
      </p>
      {path ? (
        <>
          <p class="setting-hint">Saved to {path}. Send it with one of these:</p>
          <DiscordName hint="drag the zip into a DM" />
          <div class="help-links">
            <button class="button button-quiet" onClick={() => void openIssue()}>
              GitHub issue (attach the zip)
            </button>
          </div>
        </>
      ) : (
        <div class="help-links">
          <button class="button button-primary" disabled={busy} onClick={() => void exportLogs()}>
            {busy ? "Saving…" : "Save logs"}
          </button>
        </div>
      )}
      {message && <p class="setting-hint">{message}</p>}
    </Modal>
  );
}

export function HelpSection() {
  const [modal, setModal] = useState<"contact" | "logs" | null>(null);
  const close = () => setModal(null);
  return (
    <section class="settings-section">
      <h3>Help</h3>
      <div class="setting-row">
        <span class="setting-text">
          <span class="setting-label">Contact</span>
          <span class="setting-hint">Questions, ideas or bug reports</span>
        </span>
        <span class="setting-control">
          <button class="button button-quiet" onClick={() => setModal("contact")}>
            Contact
          </button>
        </span>
      </div>
      <div class="setting-row">
        <span class="setting-text">
          <span class="setting-label">Send logs</span>
          <span class="setting-hint">Helps track down playback problems</span>
        </span>
        <span class="setting-control">
          <button class="button button-quiet" onClick={() => setModal("logs")}>
            Send logs
          </button>
        </span>
      </div>
      {modal === "contact" && <ContactModal onClose={close} />}
      {modal === "logs" && <SendLogsModal onClose={close} />}
    </section>
  );
}
