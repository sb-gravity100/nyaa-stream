import { useEffect, useRef, useState } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";
import { fallbackSearchAnime, fallbackSearchTorrentsForAnime, isTauriAvailable } from "./browserFallback";
import { getSettings } from "./settings";
import { parseSubmitterFromTitle } from "./episodeParser";
import type { AnimeMedia, NyaaResult } from "./types";
import { displayTitle } from "./types";
import "./DebugPage.css";

type Entry = { id: number; time: string; stage: string; status: "running" | "ok" | "error" | "log"; ms?: number; detail: unknown };

export function DebugPage() {
  const [query, setQuery] = useState("");
  const [anime, setAnime] = useState<AnimeMedia[]>([]);
  const [selected, setSelected] = useState<AnimeMedia | null>(null);
  const [events, setEvents] = useState<Entry[]>([]);
  const [busy, setBusy] = useState(false);
  const [filter, setFilter] = useState("all");
  const [expanded, setExpanded] = useState<number | null>(null);
  const logCursor = useRef(0);

  function record(stage: string, status: Entry["status"], detail: unknown, ms?: number) {
    const e = { id: Date.now() + Math.random(), time: clockTime(), stage, status, ms, detail };
    setEvents((old) => [...old, e]);
    return e.id;
  }

  useEffect(() => {
    const originals = { log: console.log, info: console.info, warn: console.warn, error: console.error, debug: console.debug };
    const wrap = (level: keyof typeof originals) => (...args: unknown[]) => {
      originals[level](...args);
      setEvents((old) => [...old, { id: Date.now() + Math.random(), time: clockTime(), stage: `console.${level}`, status: "log", detail: args }]);
    };
    (Object.keys(originals) as (keyof typeof originals)[]).forEach((key) => { console[key] = wrap(key); });
    record("debug session", "ok", { runtime: isTauriAvailable() ? "Tauri desktop" : "browser preview", startedAt: new Date().toISOString() });
    return () => { (Object.keys(originals) as (keyof typeof originals)[]).forEach((key) => { console[key] = originals[key]; }); };
  }, []);

  useEffect(() => {
    if (!isTauriAvailable()) return;
    let active = true;
    let pending = false;
    const poll = async () => {
      if (!active || pending) return;
      pending = true;
      try {
        const slice = await invoke<{ cursor: number; lines: string[] }>("debug_read_logs", { after: logCursor.current });
        logCursor.current = slice.cursor;
        if (active && slice.lines.length) {
          const added = slice.lines.map((line) => ({ id: Date.now() + Math.random(), time: clockTime(), stage: "backend.tracing", status: "log" as const, detail: line }));
          setEvents((old) => [...old, ...added]);
        }
      } catch (error) {
        if (active) record("backend log tail", "error", String(error));
        active = false;
      } finally { pending = false; }
    };
    void poll();
    const timer = window.setInterval(() => void poll(), 750);
    return () => { active = false; window.clearInterval(timer); };
  }, []);

  async function stage<T>(name: string, detail: unknown, run: () => Promise<T>): Promise<T | null> {
    const start = performance.now();
    record(name, "running", detail);
    try {
      const result = await run();
      record(name, "ok", result, Math.round(performance.now() - start));
      return result;
    } catch (error) {
      record(name, "error", String(error), Math.round(performance.now() - start));
      return null;
    }
  }

  async function search() {
    const q = query.trim();
    if (!q || busy) return;
    setBusy(true); setSelected(null); setAnime([]);
    const rows = await stage<AnimeMedia[]>("01 · AniList anime search", { query: q, limit: 20, transport: isTauriAvailable() ? "Tauri command → AniList, Kitsu fallback on outage" : "browser GraphQL fallback" }, () =>
      isTauriAvailable() ? invoke("search_anime", { query: q }) : fallbackSearchAnime(q));
    if (rows) setAnime(rows);
    setBusy(false);
  }

  async function select(item: AnimeMedia) {
    setSelected(item); setBusy(true);
    const args = { title: item.title, synonyms: item.synonyms ?? [], anilistId: item.id };
    const fansubber = getSettings().preferredFansubber.trim();
    record("02 · Anime selected", "ok", { id: item.id, title: item.title, format: item.format, season: item.season, year: item.seasonYear, episodes: item.episodes, synonyms: item.synonyms ?? [], note: "Production route: cache/known-anime check → navigate to #/anime/:id → render media page" });
    if (!isTauriAvailable()) {
      const releases = await stage<NyaaResult[]>("03 · Browser Nyaa fallback", { title: item.title, limitation: "Browser fallback searches English/romaji RSS; it does not run Rust title matching, local DB, or fansubber lookup." }, () => fallbackSearchTorrentsForAnime(item.title));
      record("04 · Selection enrichment", "ok", { get_anime_details: "not needed: search result already has metadata", Kitsu: "production openAnime requests get_kitsu_metadata", details: "browser preview does not scrape per-torrent view pages", releases: releases?.length ?? 0 });
      setBusy(false); return;
    }
    await Promise.all([
      stage("03a · Entry season", { anilistId: item.id }, () => invoke<number | null>("get_entry_season", args)),
      stage("03b · Episode offset", { anilistId: item.id }, () => invoke<number | null>("get_entry_episode_offset", { anilistId: item.id })),
      stage<NyaaResult[]>("03c · Local release database", args, () => invoke("search_local_releases", args)),
      ...(fansubber ? [stage<NyaaResult[]>("03d · Preferred fansubber", { ...args, fansubber }, () => invoke("search_fansubber_releases", { ...args, fansubber }))] : []),
    ]);
    const releases = await stage<NyaaResult[]>("03e · Full release search", { ...args, fansubber: fansubber || null, procedure: "load franchise → build title candidates → concurrent Nyaa searches → merge/deduplicate → show/season filter" }, () => invoke("search_torrents_for_anime", { ...args, fansubber: fansubber || null }));
    const all = releases ?? [];
    const detailsTargets = all.filter((r) => parseSubmitterFromTitle(r.title) === null || !r.label || r.label.kind === "unknown");
    await Promise.all([
      stage("04a · Anime detail refresh", { id: item.id }, () => invoke("get_anime_details", { id: item.id })),
      stage("04b · Kitsu metadata", { anilistId: item.id }, () => invoke("get_kitsu_metadata", { anilistId: item.id })),
      stage("04c · Franchise episode offset", { id: item.id }, () => invoke("get_absolute_episode_offset", { id: item.id })),
      detailsTargets.length > 0
        ? stage("04d · Ambiguous release detail scrape", { candidates: detailsTargets.map((r) => ({ title: r.title, view_url: r.view_url })), procedure: "Only titles without a recognizable group or episode/batch label are scraped; max concurrency 6." }, () => invoke("get_torrent_details_batch", { viewUrls: detailsTargets.map((r) => r.view_url) }))
        : Promise.resolve(record("04d · Ambiguous release detail scrape", "ok", { skipped: true, reason: "No result needs view-page enrichment", total: all.length })),
    ]);
    setBusy(false);
  }

  const visible = events.filter((e) => filter === "all" || (filter === "errors" ? e.status === "error" : e.stage.startsWith("console.") || e.stage.startsWith("backend.")));
  return <main class="debug-page">
    <header class="debug-head"><div><span class="debug-kicker">NYAA STREAM / DEVTOOLS</span><h1>Search flow debugger</h1><p>Trace AniList search, anime selection, release matching, and enrichment.</p></div><div class="debug-head-actions"><span class="debug-runtime">{isTauriAvailable() ? "TAURI" : "BROWSER PREVIEW"}</span><a href="#/">Exit debugger</a></div></header>
    <section class="debug-search"><form onSubmit={(e) => { e.preventDefault(); void search(); }}><input value={query} onInput={(e) => setQuery(e.currentTarget.value)} placeholder="Search an anime title…" /><button disabled={busy || !query.trim()}>{busy ? "Working…" : "Search"}</button></form><div class="debug-meta">Search is live against the current runtime. Select a result to trace its release pipeline.</div></section>
    {anime.length > 0 && <section class="debug-results"><div class="debug-section-title"><h2>AniList results</h2><span>{anime.length} returned</span></div><div class="debug-result-list">{anime.map((item, i) => <button class={selected?.id === item.id ? "debug-result selected" : "debug-result"} onClick={() => void select(item)}><b>{String(i + 1).padStart(2, "0")}</b><span class="debug-result-title">{displayTitle(item.title)}<small>{item.title.romaji} · {item.title.native}</small></span><span>{item.format} {item.seasonYear ?? ""}</span><span class="debug-arrow">↗</span></button>)}</div></section>}
    {selected && <section class="debug-selected"><div class="debug-cover">{selected.coverImage.large && <img src={selected.coverImage.large} />}</div><div><span class="debug-kicker">SELECTED ANIME · {selected.id}</span><h2>{displayTitle(selected.title)}</h2><p>{selected.title.romaji} · {selected.season ?? "—"} {selected.seasonYear ?? ""} · {selected.episodes ?? "?"} episodes</p><div class="debug-synonyms">{(selected.synonyms ?? []).slice(0, 12).map((s) => <span>{s}</span>)}</div></div></section>}
    <section class="debug-timeline"><div class="debug-section-title"><div><span class="debug-kicker">TRACE</span><h2>Procedure &amp; logs</h2></div><div class="debug-tools"><select value={filter} onChange={(e) => setFilter(e.currentTarget.value)}><option value="all">All events</option><option value="errors">Errors only</option><option value="console">Runtime logs</option></select><button onClick={() => void navigator.clipboard.writeText(JSON.stringify(events, null, 2))}>Copy JSON</button><button onClick={() => setEvents([])}>Clear</button></div></div><div class="debug-event-list">{visible.map((e) => <article class={`debug-event ${e.status}`}><button class="debug-event-row" onClick={() => setExpanded(expanded === e.id ? null : e.id)}><time>{e.time}</time><span class="debug-status">{e.status === "ok" ? "DONE" : e.status.toUpperCase()}</span><b>{e.stage}</b>{e.ms != null && <span class="debug-duration">{e.ms} ms</span>}<span class="debug-chevron">{expanded === e.id ? "−" : "+"}</span></button>{expanded === e.id && <pre>{typeof e.detail === "string" ? e.detail : JSON.stringify(e.detail, null, 2)}</pre>}</article>)}{visible.length === 0 && <div class="debug-empty">No events yet. Search a title to start a trace.</div>}</div></section>
  </main>;
}

function clockTime(): string {
  return new Date().toISOString().slice(11, 23);
}
