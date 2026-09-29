// localStorage list of the queries that led to a picked anime, newest first,
// for the search box's empty-focus dropdown.
const STORAGE_KEY = "nyaa-stream:recent-searches";
const MAX_RECENT = 8;

export function getRecentSearches(): string[] {
  try {
    const parsed = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]");
    return Array.isArray(parsed) ? parsed.filter((q): q is string => typeof q === "string") : [];
  } catch (err) {
    console.error("[recentSearches] failed to read from localStorage", { err });
    return [];
  }
}

function save(list: string[]): string[] {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(list));
  } catch (err) {
    console.error("[recentSearches] failed to write to localStorage", { err });
  }
  return list;
}

export function addRecentSearch(query: string): string[] {
  const q = query.trim();
  if (!q) return getRecentSearches();
  const list = [q, ...getRecentSearches().filter((r) => r.toLowerCase() !== q.toLowerCase())].slice(0, MAX_RECENT);
  return save(list);
}

export function removeRecentSearch(query: string): string[] {
  return save(getRecentSearches().filter((r) => r !== query));
}

export function clearRecentSearches(): string[] {
  return save([]);
}
