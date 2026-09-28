const TEXT_INPUT_TYPES = new Set(["", "text", "search", "email", "url", "password", "number", "tel"]);

/** Typing into a field - where letters and Space are text, not hotkeys.
 * Sliders and buttons don't count: clicking the volume slider or a control
 * button used to leave it focused and silently disable every hotkey. */
export function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  if (target instanceof HTMLTextAreaElement) return true;
  if (target instanceof HTMLInputElement) return TEXT_INPUT_TYPES.has(target.type.toLowerCase());
  return false;
}
