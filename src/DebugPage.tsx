import { useEffect, useMemo, useRef, useState } from "preact/hooks";
import { listen } from "@tauri-apps/api/event";
import type { AnimeMedia, NyaaResult } from "./types";
import { displayTitle } from "./types";
import { episodeLabelText, type EpisodeLabel } from "./episodeParser";
import { isTauriAvailable } from "./browserFallback";
import "./DebugPage.css";

type Outcome = "passed" | "failed" | "not needed" | "not used" | "not decisive" | "unresolved";
type Check = { name: string; outcome: Outcome; evidence: string };
export type SourceMappingRecord = {
  viewUrl: string;
  title: string;
  status: "mapped" | "filtered" | "unresolved";
  parsedLabel: string;
  mappedAs: string[];
  reason: string;
  checks: Check[];
};

type CandidateMetric = { query: string; hits: number; error: string | null };
type BackendRelease = {
  title: string;
  viewUrl: string;
  size: string;
  seeders: number;
  leechers: number;
  label: EpisodeLabel;
  episodeGroup: number | null;
  aidTrusted: boolean | null;
  queryMatches: string[];
  accepted: boolean;
  decision: string;
  reason: string;
  checks: Check[];
};
type SearchAnalytics = {
  animeId: number | null;
  entrySeason: number | null;
  queries: CandidateMetric[];
  rawHits: number;
  uniqueReleases: number;
  duplicateHits: number;
  accepted: number;
  rejected: number;
  releases: BackendRelease[];
};

type Props = {
  query: string;
  animeResults: AnimeMedia[];
  searching: boolean;
  selectedAnime: AnimeMedia | null;
  entrySeason: number;
  sources: NyaaResult[];
  sourcesLoading: boolean;
  error: string | null;
  sourceMapping: SourceMappingRecord[];
  onQueryChange: (value: string) => void;
  onSearch: (query: string) => Promise<void>;
  onSelectAnime: (anime: AnimeMedia) => void;
  onReanalyze: () => void;
};

type ReleaseRow = {
  key: string;
  title: string;
  viewUrl: string;
  size: string;
  seeders: number;
  leechers: number;
  label: string;
  accepted: boolean | null;
  decision: string;
  status: SourceMappingRecord["status"] | "rejected" | "pending";
  destinations: string[];
  reason: string;
  checks: Check[];
  queryMatches: string[];
};

export function DebugPage(props: Props) {
  const [analytics, setAnalytics] = useState<SearchAnalytics | null>(null);
  const [filter, setFilter] = useState("all");
  const [expanded, setExpanded] = useState<string | null>(null);
  const selectedId = useRef<number | null>(props.selectedAnime?.id ?? null);
  selectedId.current = props.selectedAnime?.id ?? null;

  useEffect(() => {
    if (!isTauriAvailable()) return;
    let active = true;
    let unlisten: (() => void) | undefined;
    void listen<SearchAnalytics>("source-match-analytics", (event) => {
      if (active && event.payload.animeId === selectedId.current) setAnalytics(event.payload);
    }).then((stop) => {
      if (active) unlisten = stop;
      else stop();
    });
    return () => { active = false; unlisten?.(); };
  }, []);

  const mappingByUrl = useMemo(() => new Map(props.sourceMapping.map((row) => [row.viewUrl, row])), [props.sourceMapping]);
  const backendByUrl = useMemo(() => new Map((analytics?.releases ?? []).map((row) => [row.viewUrl, row])), [analytics]);
  const rows: ReleaseRow[] = useMemo(() => {
    const records = new Map<string, ReleaseRow>();
    for (const source of props.sources) {
      const backend = backendByUrl.get(source.view_url);
      const mapping = mappingByUrl.get(source.view_url);
      records.set(source.view_url, {
        key: source.view_url,
        title: source.title,
        viewUrl: source.view_url,
        size: source.size,
        seeders: source.seeders,
        leechers: source.leechers,
        label: mapping?.parsedLabel ?? episodeLabelText(source.label ?? { kind: "unknown" }),
        accepted: backend?.accepted ?? null,
        decision: backend?.decision ?? "Awaiting backend match details",
        status: mapping?.status ?? "pending",
        destinations: mapping?.mappedAs ?? [],
        reason: mapping?.reason ?? (props.sourcesLoading ? "Search results are still arriving" : "Waiting for mapping data"),
        checks: [...(backend?.checks ?? []), ...(mapping?.checks ?? [])],
        queryMatches: backend?.queryMatches ?? [],
      });
    }
    for (const backend of analytics?.releases ?? []) {
      if (records.has(backend.viewUrl)) continue;
      records.set(backend.viewUrl, {
        key: backend.viewUrl,
        title: backend.title,
        viewUrl: backend.viewUrl,
        size: backend.size,
        seeders: backend.seeders,
        leechers: backend.leechers,
        label: episodeLabelText(backend.label),
        accepted: backend.accepted,
        decision: backend.decision,
        status: backend.accepted ? "pending" : "rejected",
        destinations: [],
        reason: backend.reason,
        checks: backend.checks,
        queryMatches: backend.queryMatches,
      });
    }
    return Array.from(records.values());
  }, [props.sources, props.sourcesLoading, mappingByUrl, backendByUrl, analytics]);

  const mapped = rows.filter((row) => row.status === "mapped").length;
  const unresolved = rows.filter((row) => row.status === "unresolved" || row.status === "pending").length;
  const filtered = rows.filter((row) => row.status === "filtered" || row.status === "rejected").length;
  const visibleRows = rows.filter((row) => filter === "all" || (filter === "mapped" ? row.status === "mapped" : filter === "left" ? row.status === "filtered" || row.status === "rejected" || row.status === "unresolved" : row.status === filter));
  const overall = analytics?.rawHits ?? props.sources.length;
  const stages = [
    { label: "Nyaa hits", value: overall, color: "pink" },
    { label: "Unique releases", value: analytics?.uniqueReleases ?? props.sources.length, color: "violet" },
    { label: "Show matched", value: analytics?.accepted ?? props.sources.length, color: "blue" },
    { label: "Episode mapped", value: mapped, color: "green" },
  ];

  return <main class="debug-page">
    <header class="debug-head">
      <div><span class="debug-kicker">SEARCH ANALYZER</span><h1>Source mapping</h1><p>Follow every release from query hit to episode row. Rejected and unresolved results stay inspectable.</p></div>
      <div class="debug-head-actions"><span class="debug-runtime">{isTauriAvailable() ? "LIVE MATCHER" : "BROWSER PREVIEW"}</span><a href="#/">Close</a></div>
    </header>

    <section class="debug-search">
      <form onSubmit={(event) => { event.preventDefault(); setAnalytics(null); setFilter("all"); void props.onSearch(props.query.trim()); }}>
        <input value={props.query} onInput={(event) => props.onQueryChange(event.currentTarget.value)} placeholder="Search anime title…" />
        <button disabled={props.searching || !props.query.trim()}>{props.searching ? "Searching…" : "Search anime"}</button>
      </form>
      {props.animeResults.length > 0 && <div class="debug-anime-results">{props.animeResults.map((anime) => (
        <button class={props.selectedAnime?.id === anime.id ? "debug-anime-choice active" : "debug-anime-choice"} onClick={() => { setAnalytics(null); setFilter("all"); props.onSelectAnime(anime); }}>
          <span>{displayTitle(anime.title)}<small>{anime.title.romaji} · {anime.title.native}</small></span><small>{anime.format} {anime.seasonYear ?? ""}</small>
        </button>
      ))}</div>}
    </section>

    {props.selectedAnime ? <>
      <section class="debug-context">
        <div class="debug-poster">{props.selectedAnime.coverImage.large && <img src={props.selectedAnime.coverImage.large} />}</div>
        <div class="debug-context-main"><span class="debug-kicker">SELECTED ENTRY · #{props.selectedAnime.id}</span><h2>{displayTitle(props.selectedAnime.title)}</h2>
          <p>{props.selectedAnime.title.english || "English title unavailable"} · {props.selectedAnime.title.romaji || "Romaji unavailable"}</p>
          <div class="debug-aliases">{(props.selectedAnime.synonyms ?? []).slice(0, 10).map((synonym) => <span>{synonym}</span>)}</div>
        </div>
        <div class="debug-season"><small>Entry season</small><b>{props.entrySeason}</b><small>{analytics?.entrySeason != null && analytics.entrySeason !== props.entrySeason ? `matcher season ${analytics.entrySeason}` : `${props.selectedAnime.episodes ?? "?"} episodes`}</small></div>
        <button class="debug-reanalyze" onClick={() => { setAnalytics(null); props.onReanalyze(); }}>{props.sourcesLoading ? "Refreshing…" : "Refresh analysis"}</button>
      </section>

      <section class="debug-overview">
        <div class="debug-overview-head"><h2>What survived each decision</h2><span>{props.error ? "Search failed" : props.sourcesLoading ? "Updating results" : analytics ? "Analysis complete" : "Waiting for backend analysis"}</span></div>
        {props.error && <div class="debug-search-error">{props.error}</div>}
        <div class="debug-funnel">{stages.map((stage, index) => <div class="debug-funnel-step">
          <div class="debug-funnel-count"><b>{stage.value.toLocaleString()}</b><span>{stage.label}</span></div>
          <div class="debug-funnel-track"><i class={stage.color} style={{ width: `${overall ? Math.max(2, stage.value / overall * 100) : 0}%` }} /></div>
          {index < stages.length - 1 && <span class="debug-funnel-chevron">›</span>}
        </div>)}</div>
        <div class="debug-stats"><span><b>{analytics?.duplicateHits ?? "—"}</b> duplicate hits removed</span><span><b>{analytics?.rejected ?? 0}</b> rejected by show match</span><span><b>{mapped}</b> mapped to page rows</span><span><b>{unresolved}</b> kept but unresolved</span></div>
      </section>

      <section class="debug-query-panel">
        <div class="debug-section-head"><div><h2>Search coverage</h2><p>Actual Nyaa query branches and their returned rows</p></div><span>{analytics?.queries.length ?? 0} queries</span></div>
        {analytics?.queries.length ? <div class="debug-query-list">{analytics.queries.map((query, index) => <div class={query.error ? "debug-query failed" : "debug-query"}>
          <b>{String(index + 1).padStart(2, "0")}</b><code>{query.query}</code><span>{query.error ? "failed" : `${query.hits.toLocaleString()} hits`}</span>
        </div>)}</div> : <div class="debug-query-empty">{props.error ?? (isTauriAvailable() ? "Select an anime to capture its live query results." : "Browser preview does not expose backend query branches.")}</div>}
      </section>

      <section class="debug-releases">
        <div class="debug-section-head"><div><h2>Release decisions</h2><p>Open a row to see each fulfilled, failed, or bypassed criterion.</p></div><div class="debug-release-counts"><span class="is-mapped">{mapped} mapped</span><span class="is-left">{filtered + unresolved} left out</span></div></div>
        <div class="debug-filter-bar">{[
          ["all", `All ${rows.length}`], ["mapped", `Mapped ${mapped}`], ["left", `Left out ${filtered + unresolved}`],
        ].map(([key, label]) => <button class={filter === key ? "active" : ""} onClick={() => setFilter(key)}>{label}</button>)}</div>
        <div class="debug-release-list">
          {visibleRows.map((row) => <article class={`debug-release ${row.status}`}>
            <button class="debug-release-main" onClick={() => setExpanded(expanded === row.key ? null : row.key)}>
              <span class={`debug-state-mark ${row.status}`}>{row.status === "mapped" ? "✓" : row.status === "rejected" || row.status === "filtered" ? "×" : "?"}</span>
              <span class="debug-release-name"><b>{row.title}</b><small>{row.label} · {row.size}</small></span>
              <span class="debug-destination">{row.destinations.length ? row.destinations.join(" · ") : row.status === "rejected" || row.status === "filtered" ? "Filtered out" : "No episode row"}</span>
              <span class="debug-seeders">↑ {row.seeders}</span><span class="debug-expand">{expanded === row.key ? "−" : "+"}</span>
            </button>
            {expanded === row.key && <div class="debug-release-detail">
              <p class="debug-reason">{row.reason}</p>
              {row.queryMatches.length > 0 && <div class="debug-release-queries"><b>Found by</b>{row.queryMatches.map((query) => <code>{query}</code>)}</div>}
              <div class="debug-criteria">{row.checks.length ? row.checks.map((check) => <div class={`debug-criterion ${check.outcome.replace(/ /g, "-")}`}>
                <span>{check.outcome === "passed" || check.outcome === "not needed" ? "✓" : check.outcome === "failed" ? "×" : "·"}</span><b>{check.name}</b><small>{check.evidence}</small>
              </div>) : <div class="debug-criteria-empty">Detailed backend checks arrive when the live search finishes.</div>}</div>
              <a href={row.viewUrl} target="_blank" rel="noreferrer">Open release page</a>
            </div>}
          </article>)}
          {!visibleRows.length && <div class="debug-no-releases">{props.sourcesLoading ? "Waiting for releases…" : "No releases in this category."}</div>}
        </div>
      </section>
    </> : <section class="debug-empty-state"><div class="debug-empty-mark">⌕</div><h2>Choose an anime to inspect</h2><p>Search above, then select a result. The analyzer will follow the real matching path and show every release that survives or gets left out.</p></section>}
  </main>;
}
