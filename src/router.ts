import { useEffect, useState } from "preact/hooks";

// Hash routes - identical under Vite's dev server and Tauri's asset
// protocol, where a real path would 404 on reload:
//   #/                          home
//   #/anime/:id                 anime page
//   #/anime/:id/episode/:n      player, a numbered episode
//   #/anime/:id/play/:key       player, any other group ("Batch", ...)
// Every navigation is a history entry, so the mouse back/forward buttons
// and Alt+Left/Right move between pages; a reload keeps the page.

/** What the player plays: a numbered episode, or a source group by key. */
export type WatchTarget = { episode: number } | { key: string };

export type Route =
  | { name: "home" }
  | { name: "anime"; id: number; watch: WatchTarget | null };

export function parseRoute(hash: string): Route {
  const parts = hash.replace(/^#\/?/, "").split("/").filter(Boolean).map(decodeURIComponent);
  if (parts[0] === "anime") {
    const id = Number(parts[1]);
    if (Number.isInteger(id) && id > 0) {
      if (parts[2] === "episode" && Number.isFinite(Number(parts[3]))) return { name: "anime", id, watch: { episode: Number(parts[3]) } };
      if (parts[2] === "play" && parts[3]) return { name: "anime", id, watch: { key: parts[3] } };
      return { name: "anime", id, watch: null };
    }
  }
  return { name: "home" };
}

export function routeHash(route: Route): string {
  if (route.name === "home") return "#/";
  const base = `#/anime/${route.id}`;
  if (!route.watch) return base;
  return "episode" in route.watch ? `${base}/episode/${route.watch.episode}` : `${base}/play/${encodeURIComponent(route.watch.key)}`;
}

/** Depth of the current entry within this app's own history, so going
 * "back" never leaves the app (e.g. right after a reload on a player
 * route). */
function depth(): number {
  const state = history.state as { depth?: number } | null;
  return typeof state?.depth === "number" ? state.depth : 0;
}

export function navigate(route: Route, { replace = false }: { replace?: boolean } = {}): void {
  const hash = routeHash(route);
  if (hash === location.hash || (hash === "#/" && location.hash === "")) return;
  console.debug("[router] navigate", { hash, replace });
  if (replace) history.replaceState({ depth: depth() }, "", hash);
  else history.pushState({ depth: depth() + 1 }, "", hash);
  // pushState/replaceState don't fire hashchange themselves.
  notify();
}

/** Back one entry, or to `fallback` when there's nothing of ours to go
 * back to. */
export function goBack(fallback: Route): void {
  if (depth() > 0) {
    console.debug("[router] back");
    history.back();
  } else {
    console.debug("[router] back with no history, replacing", { fallback: routeHash(fallback) });
    navigate(fallback, { replace: true });
  }
}

const listeners = new Set<(route: Route) => void>();

function notify() {
  const route = parseRoute(location.hash);
  for (const listener of listeners) listener(route);
}

window.addEventListener("popstate", notify);
window.addEventListener("hashchange", notify);
if (history.state == null) history.replaceState({ depth: 0 }, "", location.hash || "#/");

export function useRoute(): Route {
  const [route, setRoute] = useState(() => parseRoute(location.hash));
  useEffect(() => {
    listeners.add(setRoute);
    setRoute(parseRoute(location.hash));
    return () => {
      listeners.delete(setRoute);
    };
  }, []);
  return route;
}
