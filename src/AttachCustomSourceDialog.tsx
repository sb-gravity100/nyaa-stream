import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useRef, useState } from "preact/hooks";
import { parseEpisode, type EpisodeLabel } from "./episodeParser";
import { displayTitle, type AnimeMedia, type CustomSource } from "./types";
import "./AttachCustomSourceDialog.css";

type ParsedMagnet = { infoHash: string; name: string | null; magnet: string };
type InspectedFile = {
  path: string;
  isVideo: boolean;
  season: number | null;
  episode: number | null;
  episodeRange?: [number, number] | null;
  label?: EpisodeLabel | null;
};

export interface AttachCustomSourceDialogProps {
  input: string;
  seasonForAnime: (anime: AnimeMedia) => Promise<number>;
  onClose: () => void;
  onAttach: (source: {
    anime: AnimeMedia;
    input: string;
    title: string;
    kind: "episode" | "pack";
    season: number;
    episodeStart: number | null;
    episodeEnd: number | null;
    sourceId?: number;
    sourceAnimeId?: number;
  }) => Promise<void>;
  source?: CustomSource;
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function displayLabel(anime: AnimeMedia): string {
  return displayTitle(anime.title) || `Anime #${anime.id}`;
}

function inferredLabel(title: string): EpisodeLabel {
  return parseEpisode(title);
}

function isExtra(file: InspectedFile): boolean {
  return /\b(OVA|OAD|ONA|SP|Specials?|NCOP|NCED|Extras?|PV|Trailer)\d*\b/i.test(file.path)
    || file.label?.kind === "special"
    || (file.label?.kind === "batch" && file.label.extras === true);
}

function episodesInFile(file: InspectedFile): number[] {
  if (file.episode != null) return [file.episode];
  const range = file.episodeRange ?? (file.label?.kind === "batch" ? file.label.episodeRange : null);
  if (!range || range[1] < range[0]) return [];
  const count = Math.min(range[1] - range[0] + 1, 1000);
  return Array.from({ length: count }, (_, index) => range[0] + index);
}

export function AttachCustomSourceDialog({ input: initialInput, source, seasonForAnime, onClose, onAttach }: AttachCustomSourceDialogProps) {
  const [input, setInput] = useState(source?.magnet ?? initialInput);
  const [parsed, setParsed] = useState<ParsedMagnet | null>(null);
  const [files, setFiles] = useState<InspectedFile[] | null>(null);
  const [inspecting, setInspecting] = useState(false);
  const [inspectionError, setInspectionError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<AnimeMedia[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [anime, setAnime] = useState<AnimeMedia | null>(null);
  const [seasonLoading, setSeasonLoading] = useState(false);
  const seasonRequestId = useRef(0);
  const [kind, setKind] = useState<"episode" | "pack">(source?.kind ?? "episode");
  const [season, setSeason] = useState<number>(source?.season ?? 1);
  const [episodeStart, setEpisodeStart] = useState<number | null>(source?.episodeStart ?? null);
  const [episodeEnd, setEpisodeEnd] = useState<number | null>(source?.episodeEnd ?? null);
  const [title, setTitle] = useState(source?.title ?? "");
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const parsedTitle = parsed?.name?.trim() ?? "";
  const titleLabel = useMemo(() => parsedTitle ? inferredLabel(parsedTitle) : { kind: "unknown" } as const, [parsedTitle]);
  const eligibleFiles = files?.filter((file) => file.isVideo && !isExtra(file)) ?? [];
  const inferredPack = eligibleFiles.length >= 2 || eligibleFiles.some((file) => episodesInFile(file).length > 1);
  const hasNumberedFiles = eligibleFiles.some((file) => episodesInFile(file).length > 0);
  const packRangeInvalid = episodeStart === null !== (episodeEnd === null)
    || (episodeStart !== null && episodeEnd !== null && (!Number.isInteger(episodeStart) || !Number.isInteger(episodeEnd) || episodeStart < 1 || episodeEnd < episodeStart));

  useEffect(() => {
    let cancelled = false;
    const value = input.trim();
    setParsed(null);
    setFiles(null);
    setInspecting(false);
    setInspectionError(null);
    if (!value) return;
    const timer = window.setTimeout(async () => {
      setInspecting(true);
      try {
        const result = await invoke<ParsedMagnet>("parse_custom_magnet", { input: value });
        if (cancelled) return;
        setParsed(result);
        if (!source && !title.trim()) setTitle(result.name?.trim() ?? "");
        const inspected = await invoke<InspectedFile[]>("inspect_custom_magnet", { input: value });
        if (cancelled) return;
        setFiles(inspected);
        const parsedLabel = result.name ? inferredLabel(result.name) : { kind: "unknown" } as const;
        if (!source) {
          const eligibleMetadata = inspected.filter((file) => file.isVideo && !isExtra(file));
          if (eligibleMetadata.length >= 2 || eligibleMetadata.some((file) => episodesInFile(file).length > 1)) {
            setKind("pack");
            if (parsedLabel.kind === "batch" && !parsedLabel.extras && parsedLabel.episodeRange) {
              setSeason(parsedLabel.season);
              setEpisodeStart(parsedLabel.episodeRange[0]);
              setEpisodeEnd(parsedLabel.episodeRange[1]);
            } else {
              const fileEpisodes = inspected.filter((file) => file.isVideo && !isExtra(file)).flatMap(episodesInFile);
              setEpisodeStart(fileEpisodes.length ? Math.min(...fileEpisodes) : null);
              setEpisodeEnd(fileEpisodes.length ? Math.max(...fileEpisodes) : null);
            }
          } else if (parsedLabel.kind === "episode") {
            setKind("episode");
            setSeason(parsedLabel.season);
            setEpisodeStart(parsedLabel.number);
            setEpisodeEnd(parsedLabel.number);
          } else {
            const onlyVideo = inspected.find((file) => file.isVideo && !isExtra(file));
            setKind("episode");
            if (onlyVideo?.season != null) setSeason(onlyVideo.season);
            const episodeNumber = onlyVideo?.episode ?? null;
            setEpisodeStart(episodeNumber);
            setEpisodeEnd(episodeNumber);
          }
        }
      } catch (error) {
        if (!cancelled) {
          setInspectionError(errorText(error));
          setParsed(null);
          setFiles(null);
        }
      } finally {
        if (!cancelled) setInspecting(false);
      }
    }, 250);
    return () => { cancelled = true; window.clearTimeout(timer); };
  }, [input, source]);

  useEffect(() => {
    const value = query.trim();
    setSearchError(null);
    if (value.length < 2) { setResults([]); return; }
    let cancelled = false;
    const timer = window.setTimeout(async () => {
      setSearching(true);
      try {
        const found = await invoke<AnimeMedia[]>("search_anime", { query: value });
        if (!cancelled) setResults(found.slice(0, 8));
      } catch (error) {
        if (!cancelled) { setSearchError(errorText(error)); setResults([]); }
      } finally {
        if (!cancelled) setSearching(false);
      }
    }, 300);
    return () => { cancelled = true; window.clearTimeout(timer); };
  }, [query]);

  async function chooseAnime(candidate: AnimeMedia) {
    const requestId = ++seasonRequestId.current;
    setAnime(candidate);
    if (source?.animeId === candidate.id && source.season != null) {
      setSeason(source.season);
      setSeasonLoading(false);
    } else {
      setSeasonLoading(true);
      try {
        const candidateSeason = await seasonForAnime(candidate);
        if (requestId === seasonRequestId.current) setSeason(candidateSeason);
      } finally {
        if (requestId === seasonRequestId.current) setSeasonLoading(false);
      }
    }
    setResults([]);
    setQuery(displayLabel(candidate));
  }

  async function submit(event: Event) {
    event.preventDefault();
    if (!anime || !parsed || !files || inspecting || seasonLoading || saving) return;
    if (kind === "episode" && (episodeStart === null || !Number.isInteger(episodeStart) || episodeStart < 1)) return;
    const hasRange = episodeStart != null || episodeEnd != null;
    if (kind === "pack" && hasRange && packRangeInvalid) return;
    setSaving(true);
    setSaveError(null);
    try {
      await onAttach({
        anime, input: parsed.magnet, title: title.trim() || parsed.name?.trim() || `Torrent ${parsed.infoHash.slice(0, 8)}`,
        kind, season, episodeStart,
        episodeEnd: kind === "episode" ? episodeStart : episodeEnd,
        ...(source ? { sourceId: source.id, sourceAnimeId: source.animeId } : {}),
      });
      onClose();
    } catch (error) {
      setSaveError(errorText(error));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div class="export-backdrop attach-source-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <form class="export-dialog attach-source-dialog" role="dialog" aria-modal="true" aria-labelledby="attach-source-title" onSubmit={submit} onKeyDown={(event) => { if (event.key === "Escape") onClose(); }}>
        <div class="attach-source-heading">
          <div><h2 id="attach-source-title">{source ? "Move custom source" : "Attach custom source"}</h2><p>Inspect a torrent, then match it to an anime.</p></div>
          <button type="button" class="attach-source-close" onClick={onClose} aria-label="Close">×</button>
        </div>

        <label class="attach-source-field">Magnet link or info hash
          <textarea value={input} onInput={(event) => setInput((event.currentTarget as HTMLTextAreaElement).value)} rows={3} spellcheck={false} />
        </label>
        <div class="attach-source-status" aria-live="polite">
          {inspecting ? "Inspecting torrent metadata…" : parsed && files ? `${eligibleFiles.length} eligible video files · ${parsed.infoHash.slice(0, 12)}…` : inspectionError ? `Inspection failed: ${inspectionError}` : "Enter a magnet link or info hash."}
        </div>
        {files && eligibleFiles.length === 0 && <div class="attach-source-error">No episode video files were found. Extras and specials are excluded.</div>}

        <label class="attach-source-field">Search AniList
          <input value={query} onInput={(event) => { setQuery((event.currentTarget as HTMLInputElement).value); setAnime(null); }} placeholder="Anime title" autocomplete="off" />
        </label>
        {searching && <div class="attach-source-status">Searching AniList…</div>}
        {searchError && <div class="attach-source-error">Search failed: {searchError}</div>}
        {results.length > 0 && <div class="attach-source-results" role="listbox" aria-label="Anime results">
          {results.map((candidate) => <button type="button" role="option" aria-selected={anime?.id === candidate.id} key={candidate.id} onClick={() => chooseAnime(candidate)}>
            {candidate.coverImage.large && <img src={candidate.coverImage.large} alt="" />}
            <span>{displayLabel(candidate)}{candidate.seasonYear ? ` (${candidate.seasonYear})` : ""}</span>
          </button>)}
        </div>}
        {anime && <div class="attach-source-selected">Selected: <strong>{displayLabel(anime)}</strong></div>}

        <label class="attach-source-field">Source title
          <input value={title} onInput={(event) => setTitle((event.currentTarget as HTMLInputElement).value)} maxlength={200} placeholder="Torrent title" />
        </label>
        <div class="attach-source-options">
          <label class="attach-source-field">Type
            <select value={kind} onChange={(event) => setKind((event.currentTarget as HTMLSelectElement).value as "episode" | "pack")}>
              <option value="episode">Episode</option><option value="pack">Pack</option>
            </select>
          </label>
          <label class="attach-source-field">Season
            <input type="number" min="1" value={season} onInput={(event) => setSeason(Number((event.currentTarget as HTMLInputElement).value))} />
          </label>
        </div>
        {kind === "episode" ? <label class="attach-source-field">Episode
          <input type="number" min="1" value={episodeStart ?? ""} onInput={(event) => setEpisodeStart(event.currentTarget.value === "" ? null : Number(event.currentTarget.value))} />
        </label> : <div class="attach-source-options">
          <label class="attach-source-field">Range start (optional)<input type="number" min="1" value={episodeStart ?? ""} onInput={(event) => setEpisodeStart(event.currentTarget.value === "" ? null : Number(event.currentTarget.value))} /></label>
          <label class="attach-source-field">Range end (optional)<input type="number" min="1" value={episodeEnd ?? ""} onInput={(event) => setEpisodeEnd(event.currentTarget.value === "" ? null : Number(event.currentTarget.value))} /></label>
        </div>}
        {titleLabel.kind === "episode" && !source && <div class="attach-source-status">Episode {titleLabel.number} inferred from the torrent name.</div>}
        {inferredPack && !source && <div class="attach-source-status">Pack inferred from video files; extras are excluded.</div>}
        {kind === "pack" && eligibleFiles.length > 0 && !hasNumberedFiles && <div class="attach-source-error">No episode numbers were found in the files. This pack will remain on the Batch row.</div>}
        {saveError && <div class="attach-source-error">Couldn’t attach source: {saveError}</div>}
        <div class="attach-source-actions">
          <button type="button" class="button button-secondary" onClick={onClose}>Cancel</button>
          <button type="submit" class="button" disabled={!parsed || !files || eligibleFiles.length === 0 || inspecting || seasonLoading || !anime || saving || !Number.isInteger(season) || season < 1 || (kind === "episode" && (episodeStart === null || !Number.isInteger(episodeStart) || episodeStart < 1)) || (kind === "pack" && packRangeInvalid)}>{saving ? "Attaching…" : seasonLoading ? "Loading season…" : source ? "Move source" : "Attach source"}</button>
        </div>
      </form>
    </div>
  );
}
