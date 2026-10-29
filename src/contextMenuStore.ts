import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { openUrl } from "@tauri-apps/plugin-opener";
import { isTypingTarget } from "./keyboard";

export type ContextMenuItem = {
  label?: string;
  shortcut?: string;
  disabled?: boolean;
  separator?: boolean;
  action?: () => void | Promise<void>;
};

export type ContextMenuState = { x: number; y: number; items: ContextMenuItem[] };

type ContextMenuListener = (state: ContextMenuState | null) => void;

let currentMenu: ContextMenuState | null = null;
const listeners = new Set<ContextMenuListener>();

export function getContextMenuSnapshot(): ContextMenuState | null {
  console.debug("[context-menu] read snapshot", { open: currentMenu !== null });
  return currentMenu;
}

export function subscribeContextMenu(listener: ContextMenuListener): () => void {
  console.debug("[context-menu] subscribe", { listeners: listeners.size + 1 });
  listeners.add(listener);
  return () => {
    console.debug("[context-menu] unsubscribe", { listeners: listeners.size - 1 });
    listeners.delete(listener);
  };
}

function publishContextMenu(reason: string): void {
  console.debug("[context-menu] publish", { reason, open: currentMenu !== null, listeners: listeners.size });
  for (const listener of listeners) listener(currentMenu);
}

export function closeContextMenu(reason = "dismissed"): void {
  if (!currentMenu) return;
  console.debug("[context-menu] close", { reason, items: currentMenu.items.length });
  currentMenu = null;
  publishContextMenu(reason);
}

export function openContextMenu(event: MouseEvent, items: ContextMenuItem[]): void {
  console.debug("[context-menu] open requested", { x: event.clientX, y: event.clientY, items: items.length, shift: event.shiftKey });
  if (import.meta.env.DEV && event.shiftKey) return;
  event.preventDefault();
  event.stopPropagation();
  if (!items.some((item) => !item.separator)) {
    closeContextMenu("empty-menu");
    return;
  }
  currentMenu = { x: event.clientX, y: event.clientY, items };
  publishContextMenu("opened");
}

export async function copyToClipboard(text: string, purpose: string): Promise<void> {
  console.info("[context-menu] copy requested", { purpose, length: text.length });
  try {
    await writeText(text);
    console.debug("[context-menu] copy completed", { purpose, length: text.length });
  } catch (error) {
    console.error("[context-menu] copy failed", { purpose, error: String(error) });
  }
}

export async function openAniListPage(animeId: number): Promise<void> {
  const url = `https://anilist.co/anime/${animeId}`;
  console.info("[context-menu] open AniList page", { animeId });
  try {
    await openUrl(url);
  } catch (error) {
    console.error("[context-menu] AniList open failed", { animeId, error: String(error) });
  }
}

function textSelection(target: HTMLElement): { text: string; start?: number; end?: number } {
  if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
    const start = target.selectionStart ?? 0;
    const end = target.selectionEnd ?? start;
    const text = target.value.slice(start, end);
    console.debug("[context-menu] input selection", { length: text.length, start, end });
    return { text, start, end };
  }
  const text = window.getSelection()?.toString() ?? "";
  console.debug("[context-menu] editable selection", { length: text.length });
  return { text };
}

function canEdit(target: HTMLElement): boolean {
  const editable = isTypingTarget(target)
    && !(target instanceof HTMLInputElement && (target.readOnly || target.disabled))
    && !(target instanceof HTMLTextAreaElement && (target.readOnly || target.disabled));
  console.debug("[context-menu] editability", { editable, tag: target.tagName });
  return editable;
}

function dispatchInput(target: HTMLElement, text: string): void {
  console.debug("[context-menu] dispatch input", { tag: target.tagName, length: text.length });
  target.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: text }));
}

async function copySelection(target: HTMLElement, cut: boolean): Promise<void> {
  const selection = textSelection(target);
  if (!selection.text) {
    console.debug("[context-menu] copy skipped", { reason: "empty-selection", cut });
    return;
  }
  console.debug("[context-menu] clipboard write requested", { length: selection.text.length, cut });
  try {
    await writeText(selection.text);
  } catch (error) {
    console.error("[context-menu] clipboard write failed", { cut, error: String(error) });
    return;
  }
  if (!cut) return;
  if ((target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) && selection.start !== undefined && selection.end !== undefined) {
    target.focus();
    target.setRangeText("", selection.start, selection.end, "start");
    dispatchInput(target, "");
    return;
  }
  const activeSelection = window.getSelection();
  if (activeSelection?.rangeCount) {
    activeSelection.deleteFromDocument();
    dispatchInput(target, "");
  }
}

async function pasteInto(target: HTMLElement): Promise<void> {
  console.debug("[context-menu] clipboard read requested");
  let text: string;
  try {
    text = await readText();
  } catch (error) {
    console.error("[context-menu] clipboard read failed", { error: String(error) });
    return;
  }
  console.debug("[context-menu] clipboard read complete", { length: text.length });
  target.focus();
  if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
    const start = target.selectionStart ?? target.value.length;
    const end = target.selectionEnd ?? start;
    target.setRangeText(text, start, end, "end");
    dispatchInput(target, text);
    return;
  }
  const selection = window.getSelection();
  if (!selection) {
    console.warn("[context-menu] paste skipped", { reason: "no-selection" });
    return;
  }
  let range: Range;
  if (selection.rangeCount) range = selection.getRangeAt(0);
  else {
    range = document.createRange();
    range.selectNodeContents(target);
    range.collapse(false);
  }
  range.deleteContents();
  const textNode = document.createTextNode(text);
  range.insertNode(textNode);
  range.setStartAfter(textNode);
  range.collapse(true);
  selection.removeAllRanges();
  selection.addRange(range);
  dispatchInput(target, text);
}

function selectAll(target: HTMLElement): void {
  console.debug("[context-menu] select all", { tag: target.tagName });
  target.focus();
  if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
    target.select();
    return;
  }
  const selection = window.getSelection();
  const range = document.createRange();
  range.selectNodeContents(target);
  selection?.removeAllRanges();
  selection?.addRange(range);
}

function textInputItems(target: HTMLElement): ContextMenuItem[] {
  console.debug("[context-menu] build text-input menu", { tag: target.tagName });
  const selected = textSelection(target);
  const disabled = !canEdit(target);
  const password = target instanceof HTMLInputElement && target.type.toLowerCase() === "password";
  return [
    { label: "Cut", shortcut: "Ctrl+X", disabled: disabled || password || !selected.text, action: () => copySelection(target, true) },
    { label: "Copy", shortcut: "Ctrl+C", disabled: password || !selected.text, action: () => copySelection(target, false) },
    { label: "Paste", shortcut: "Ctrl+V", disabled, action: () => pasteInto(target) },
    { separator: true },
    { label: "Select all", shortcut: "Ctrl+A", action: () => selectAll(target) },
  ];
}

function selectedTextItems(selectionText: string): ContextMenuItem[] {
  console.debug("[context-menu] build selection menu", { length: selectionText.length });
  return [{ label: "Copy", shortcut: "Ctrl+C", action: async () => {
    console.debug("[context-menu] copy selection", { length: selectionText.length });
    try {
      await writeText(selectionText);
    } catch (error) {
      console.error("[context-menu] selection copy failed", { error: String(error) });
    }
  } }];
}

export function installGlobalContextMenu(): () => void {
  console.info("[context-menu] install global handler");
  const handleContextMenu = (event: MouseEvent) => {
    const target = event.target instanceof HTMLElement ? event.target : null;
    const selection = window.getSelection()?.toString() ?? "";
    console.debug("[context-menu] global right-click", { tag: target?.tagName ?? "unknown", shift: event.shiftKey, selectionLength: selection.length });
    if (import.meta.env.DEV && event.shiftKey) return;
    if (target && isTypingTarget(target)) {
      openContextMenu(event, textInputItems(target));
      return;
    }
    if (selection) {
      openContextMenu(event, selectedTextItems(selection));
      return;
    }
    event.preventDefault();
    closeContextMenu("no-menu-for-target");
  };
  window.addEventListener("contextmenu", handleContextMenu);
  return () => {
    console.info("[context-menu] remove global handler");
    window.removeEventListener("contextmenu", handleContextMenu);
  };
}
