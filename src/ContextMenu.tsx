import { useEffect, useRef, useState } from "preact/hooks";
import { closeContextMenu, getContextMenuSnapshot, subscribeContextMenu, type ContextMenuItem, type ContextMenuState } from "./contextMenuStore";
import "./ContextMenu.css";

function nextEnabledIndex(items: ContextMenuItem[], start: number, direction: 1 | -1): number {
  console.debug("[context-menu] find keyboard item", { start, direction, items: items.length });
  for (let step = 1; step <= items.length; step += 1) {
    const index = (start + direction * step + items.length) % items.length;
    if (!items[index].separator && !items[index].disabled) return index;
  }
  return -1;
}

export function ContextMenu() {
  const [menu, setMenu] = useState<ContextMenuState | null>(getContextMenuSnapshot);
  const [position, setPosition] = useState({ left: 0, top: 0, ready: false });
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    console.debug("[context-menu] component mounted");
    const unsubscribe = subscribeContextMenu(setMenu);
    return () => {
      console.debug("[context-menu] component unmounted");
      unsubscribe();
    };
  }, []);

  useEffect(() => {
    if (!menu) return;
    console.debug("[context-menu] lifecycle listeners added", { items: menu.items.length });
    setPosition({ left: menu.x, top: menu.y, ready: false });
    const element = menuRef.current;
    if (element) {
      const bounds = element.getBoundingClientRect();
      const left = Math.max(8, Math.min(menu.x, window.innerWidth - bounds.width - 8));
      const top = Math.max(8, Math.min(menu.y, window.innerHeight - bounds.height - 8));
      console.debug("[context-menu] clamp position", { x: menu.x, y: menu.y, left, top, width: bounds.width, height: bounds.height });
      setPosition({ left, top, ready: true });
      element.querySelector<HTMLButtonElement>('button[role="menuitem"]:not(:disabled)')?.focus();
    }
    const closeForOutsidePointer = (event: PointerEvent) => {
      const inside = !!menuRef.current?.contains(event.target as Node);
      console.debug("[context-menu] pointer down", { inside });
      if (!inside) closeContextMenu("outside-click");
    };
    const closeForScroll = () => {
      console.debug("[context-menu] dismissed by scroll");
      closeContextMenu("scroll");
    };
    const closeForBlur = () => {
      console.debug("[context-menu] dismissed by window blur");
      closeContextMenu("window-blur");
    };
    const closeForNavigation = () => {
      console.debug("[context-menu] dismissed by navigation");
      closeContextMenu("navigation");
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      console.debug("[context-menu] keydown", { key: event.key });
      if (event.key === "Escape") {
        event.preventDefault();
        closeContextMenu("escape");
        return;
      }
      if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
      event.preventDefault();
      const buttons = Array.from(menuRef.current?.querySelectorAll<HTMLButtonElement>('button[role="menuitem"]:not(:disabled)') ?? []);
      const activeButton = document.activeElement as HTMLButtonElement;
      const activeIndex = Number(activeButton.dataset.itemIndex ?? -1);
      const direction = event.key === "ArrowDown" ? 1 : -1;
      const next = activeIndex < 0 ? (direction === 1 ? 0 : buttons.length - 1) : nextEnabledIndex(menu.items, activeIndex, direction);
      buttons.find((button) => Number(button.dataset.itemIndex) === next)?.focus();
    };
    window.addEventListener("pointerdown", closeForOutsidePointer, true);
    window.addEventListener("scroll", closeForScroll, true);
    window.addEventListener("blur", closeForBlur);
    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("popstate", closeForNavigation);
    window.addEventListener("hashchange", closeForNavigation);
    window.addEventListener("nyaa-stream:navigate", closeForNavigation);
    return () => {
      console.debug("[context-menu] lifecycle listeners removed");
      window.removeEventListener("pointerdown", closeForOutsidePointer, true);
      window.removeEventListener("scroll", closeForScroll, true);
      window.removeEventListener("blur", closeForBlur);
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("popstate", closeForNavigation);
      window.removeEventListener("hashchange", closeForNavigation);
      window.removeEventListener("nyaa-stream:navigate", closeForNavigation);
    };
  }, [menu]);

  if (!menu) return null;

  const selectItem = async (item: ContextMenuItem) => {
    console.info("[context-menu] item selected", { label: item.label ?? "separator", disabled: !!item.disabled });
    if (item.disabled || item.separator || !item.action) return;
    closeContextMenu("item-selected");
    try {
      await item.action();
    } catch (error) {
      console.error("[context-menu] item action failed", { label: item.label, error: String(error) });
    }
  };

  return (
    <div
      ref={menuRef}
      class="context-menu"
      role="menu"
      aria-label="Context menu"
      style={{ left: `${position.left}px`, top: `${position.top}px`, visibility: position.ready ? "visible" : "hidden" }}
    >
      {menu.items.map((item, index) => item.separator ? (
        <div class="context-menu-separator" role="separator" key={`separator-${index}`} />
      ) : (
        <button
          class="context-menu-item"
          type="button"
          role="menuitem"
          data-item-index={index}
          disabled={item.disabled}
          key={`${item.label}-${index}`}
          onClick={() => { console.debug("[context-menu] click", { label: item.label }); void selectItem(item); }}
        >
          <span>{item.label}</span>
          {item.shortcut && <span class="context-menu-shortcut">{item.shortcut}</span>}
        </button>
      ))}
    </div>
  );
}
