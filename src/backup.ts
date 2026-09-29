import { invoke } from "@tauri-apps/api/core";

// Everything the app keeps in localStorage (library, watch progress,
// settings, preferred groups...) - all keys share this prefix. A webview
// data clear wipes them, so Settings can write them to a file.
const PREFIX = "nyaa-stream:";
const FORMAT = 1;

interface Backup {
  app: "nyaa-stream";
  format: number;
  exportedAt: string;
  data: Record<string, string>;
}

/** Writes a backup file and returns where it went. */
export async function exportBackup(): Promise<string> {
  const data: Record<string, string> = {};
  for (let i = 0; i < localStorage.length; i++) {
    const key = localStorage.key(i);
    if (key?.startsWith(PREFIX)) data[key] = localStorage.getItem(key) ?? "";
  }
  const backup: Backup = { app: "nyaa-stream", format: FORMAT, exportedAt: new Date().toISOString(), data };
  console.info("[backup] exporting", { keys: Object.keys(data).length });
  return invoke<string>("export_backup", { json: JSON.stringify(backup, null, 2) });
}

/** Restores the backup file, replacing what's stored, and returns how many
 * entries it held. The caller reloads so every store rereads its data. */
export async function importBackup(): Promise<number> {
  const backup = JSON.parse(await invoke<string>("import_backup")) as Partial<Backup>;
  if (backup.app !== "nyaa-stream" || !backup.data || typeof backup.data !== "object") throw new Error("That file isn't a nyaa-stream backup.");
  const entries = Object.entries(backup.data).filter(([key, value]) => key.startsWith(PREFIX) && typeof value === "string");
  for (const [key, value] of entries) localStorage.setItem(key, value);
  console.info("[backup] imported", { keys: entries.length });
  return entries.length;
}
