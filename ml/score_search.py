"""Score a live nyaa.si search against one franchise with the trained XGBoost model.

usage: score_search.py <model.json> <entries.json (tf-idf vocabulary only)> <fribb.json> <results.jsonl>
                       <anilist search text> <target anilist id> [--out table.tsv]
No AnimeTosho data: the franchise entries come from AniList (relations walk) + Fribb (TVDB season),
the releases from `search_json` (live nyaa.si). The candidate pool is the franchise only. The
feature code mirrors train.py `build` (keep in sync). Scratch, not shipped.
"""
import argparse, json, re, subprocess, time, urllib.request, datetime as dt, collections, os
import numpy as np
import xgboost as xgb
from rapidfuzz import fuzz
from sklearn.feature_extraction.text import TfidfVectorizer

ap = argparse.ArgumentParser()
for a in ("model", "entries", "fribb", "results", "search", "target"): ap.add_argument(a)
ap.add_argument("--out", default=""); ap.add_argument("--cache", default="franchise_cache.json")
ap.add_argument("--json-out", default="", help="write per-release debugger predictions")
ap.add_argument("--parser", default=r"C:\Users\SB\Desktop\Projects\nyaa_stream\target\release\examples\ml_parse.exe")
args = ap.parse_args(); target = int(args.target)

def norm(s):
    s = (s or "").lower(); s = re.sub(r"[^0-9a-z\u3040-\u30ff\u4e00-\u9fff\u0400-\u04ff]+", " ", s)
    return re.sub(r"\s+", " ", s).strip()
def day(d):
    try: return dt.date(d["year"], d["month"] or 6, d["day"] or 15).toordinal() if d and d.get("year") else None
    except Exception: return None

def gql(q, v):
    body = json.dumps({"query": q, "variables": v}).encode()
    for k in range(5):
        try:
            r = urllib.request.urlopen(urllib.request.Request("https://graphql.anilist.co", body,
                {"Content-Type": "application/json", "Accept": "application/json", "User-Agent": "nyaa-stream-ml/0.1"}), timeout=60)
            return json.load(r)["data"]
        except Exception as e: print("retry", e); time.sleep(8 * (k + 1))
    raise SystemExit("AniList unreachable")

# ---- franchise from AniList (cached) ---------------------------------------
if os.path.exists(args.cache):
    fran = {int(k): v for k, v in json.load(open(args.cache, encoding="utf8")).items()}
else:
    F = "id format episodes startDate{year month day} endDate{year month day} title{romaji english native} synonyms relations{edges{relationType node{id type}}}"
    seed = gql("query($s:String){Media(search:$s,type:ANIME){id}}", {"s": args.search})["Media"]["id"]
    fran, todo = {}, [seed]
    while todo:
        ids = [i for i in todo if i not in fran][:50]; todo = [i for i in todo if i not in fran and i not in ids]
        if not ids: break
        for m in gql("query($ids:[Int]){Page(perPage:50){media(id_in:$ids,type:ANIME){%s}}}" % F, {"ids": ids})["Page"]["media"]:
            fran[m["id"]] = m
            todo += [e["node"]["id"] for e in m["relations"]["edges"] if e["node"]["type"] == "ANIME" and e["node"]["id"] not in fran
                     and e["relationType"] in ("PREQUEL", "SEQUEL", "SIDE_STORY", "PARENT", "SUMMARY", "ALTERNATIVE")]
        time.sleep(1.2)
    json.dump(fran, open(args.cache, "w", encoding="utf8"))
fr = {e["anilist_id"]: e for e in json.load(open(args.fribb, encoding="utf8")) if e.get("anilist_id")}
FORMATS = ["TV", "TV_SHORT", "MOVIE", "SPECIAL", "OVA", "ONA", "MUSIC"]
ent = {}
for lid, m in fran.items():
    names = sorted({norm(x) for x in [m["title"].get("romaji"), m["title"].get("english"), m["title"].get("native"), *(m.get("synonyms") or [])] if x} - {""})
    f = fr.get(lid, {})
    ent[lid] = dict(names=names, tvdb=f.get("tvdb_id"), season=(f.get("season") or {}).get("tvdb"),
                    fmt=FORMATS.index(m["format"]) if m.get("format") in FORMATS else -1, eps=m.get("episodes"),
                    start=day(m.get("startDate")), end=day(m.get("endDate")), label=m["title"].get("romaji") or str(lid))
by_tvdb = collections.defaultdict(list)
for a, e in ent.items():
    if e["tvdb"]: by_tvdb[e["tvdb"]].append(a)
print(f"franchise entries ({len(ent)}):")
for a, e in sorted(ent.items(), key=lambda kv: kv[1]["start"] or 0):
    print(f"   {a:<7} {e['label'][:48]:<48} fmt={FORMATS[e['fmt']] if e['fmt']>=0 else '?':<8} eps={e['eps']} tvdb_season={e['season']} start={dt.date.fromordinal(e['start']) if e['start'] else None}")

# ---- releases + parser ------------------------------------------------------
rel = [json.loads(l) for l in open(args.results, encoding="utf8")]
titles = "\n".join(r["title"].replace("\n", " ") for r in rel) + "\n"
parsed = [json.loads(l) for l in subprocess.run([args.parser], input=titles.encode("utf8"), capture_output=True, check=True).stdout.decode("utf8").splitlines()]
for r, p in zip(rel, parsed):
    r.update({k: v for k, v in p.items() if k != "title"}); r["show"] = norm(p.get("title") or r["title"])
    r["day"] = dt.date.fromisoformat(r["published"][:10]).toordinal() if re.match(r"\d{4}-\d\d-\d\d", r.get("published", "")) else None
rel = [r for r in rel if r["day"]]

# ---- tf-idf as in training (vocabulary from the training pool) ---------------
E0 = json.load(open(args.entries, encoding="utf8")); al0 = E0["anilist"]
vocab = []
for m in al0.values():
    vocab += [norm(x) for x in [m["title"].get("romaji"), m["title"].get("english"), m["title"].get("native"), *(m.get("synonyms") or [])] if x]
tf = TfidfVectorizer(analyzer="char_wb", ngram_range=(3, 4), min_df=1, sublinear_tf=True, dtype=np.float32).fit([v for v in vocab if v])
names, name_aid = [], []
for a, e in ent.items():
    for n in e["names"]: names.append(n); name_aid.append(a)
NM = tf.transform(names)

FEATS = ["ts", "r", "pr", "tfidf", "sim_rank", "sim_gap", "n_cand", "n_sib", "rel_has_season", "rel_season", "ent_season",
         "season_match", "season_diff", "ent_s1", "d_start", "d_end", "in_window", "ep_first", "ep_last", "ent_eps",
         "ep_fits", "ep_over", "is_batch", "rel_kind", "rel_extras", "ent_fmt", "movie_match", "n_names", "best_ts_all"]
KIND = {"episode": 0, "batch": 1, "movie": 2, "special": 3, "unknown": 4}
nan = float("nan")
bst = xgb.Booster(); bst.load_model(args.model)
cands = list(ent)
rows, owner = [], []
for gi, r in enumerate(rel):
    v = tf.transform([r["show"]]) @ NM.T; v = v.toarray()[0]
    tfd = {a: 0.0 for a in cands}
    for k, a in enumerate(name_aid): tfd[a] = max(tfd[a], float(v[k]))
    sims = {a: (max(fuzz.token_set_ratio(r["show"], n) for n in ent[a]["names"]), max(fuzz.ratio(r["show"], n) for n in ent[a]["names"]),
                max(fuzz.partial_ratio(r["show"], n) for n in ent[a]["names"])) for a in cands}
    order = sorted(cands, key=lambda a: -sims[a][0]); best = sims[order[0]][0]
    ep = r.get("episodes"); seasons = r.get("seasons") or []
    for a in cands:
        e = ent[a]; ts, rt, pr = sims[a]; es = e["season"]; rs = seasons[0] if seasons else nan
        sm = 1.0 if (es is not None and es in seasons) else (0.0 if seasons and es is not None else nan)
        ef, el = (ep[0], ep[1]) if ep else (nan, nan); eps = e["eps"] if e["eps"] else nan
        rows.append([ts, rt, pr, tfd[a], order.index(a), best - ts, len(cands), len(by_tvdb.get(e["tvdb"], [])) if e["tvdb"] else 0,
                     1.0 if seasons else 0.0, rs, es if es is not None else nan, sm, (rs - es) if (seasons and es is not None) else nan,
                     1.0 if es == 1 else 0.0, (r["day"] - e["start"]) if e["start"] else nan, (r["day"] - e["end"]) if e["end"] else nan,
                     1.0 if e["start"] and r["day"] >= e["start"] - 21 and (not e["end"] or r["day"] <= e["end"] + 400) else 0.0,
                     ef, el, eps, (1.0 if el <= eps else 0.0) if (ep and eps == eps) else nan, (el - eps) if (ep and eps == eps) else nan,
                     1.0 if r.get("kind") == "batch" else 0.0, KIND.get(r.get("kind"), 4), 1.0 if r.get("extras") else 0.0,
                     e["fmt"], 1.0 if (r.get("kind") == "movie") == (e["fmt"] == 2) else 0.0, len(e["names"]), best])
        owner.append((gi, a))
P = bst.predict(xgb.DMatrix(np.array(rows, dtype=np.float32), feature_names=FEATS))
res = {}
for (gi, a), p in zip(owner, P): res.setdefault(gi, []).append((float(p), a))
out = []
for gi, r in enumerate(rel):
    s = sorted(res[gi], reverse=True); b, top = s[0]; m = b - (s[1][0] if len(s) > 1 else 0)
    out.append((r, top if (b >= 0.95 and m >= 0.3) else None, b, top))
if args.json_out:
    with open(args.json_out, "w", encoding="utf8") as f:
        json.dump([{"view_url": r.get("view_url"), "predicted_entry": assigned,
                    "top_entry": top, "confidence": confidence,
                    "margin": round(sorted(res[i], reverse=True)[0][0] -
                                    (sorted(res[i], reverse=True)[1][0] if len(res[i]) > 1 else 0), 6),
                    "target_match": assigned == target if assigned is not None else None}
                   for i, (r, assigned, confidence, top) in enumerate(out)], f)

# ---- report -----------------------------------------------------------------
cnt = collections.Counter(ent[t]["label"] if t else "(abstain / not this franchise)" for _, t, _, _ in out)
print(f"\n{len(out)} scraped releases:")
for k, v in cnt.most_common(): print(f"   {v:>5}  {k}")
mine = [(r, b) for r, t, b, _ in out if t == target]
print(f"\nassigned to {ent[target]['label']} [{target}]: {len(mine)}")
eps = collections.defaultdict(list); packs = []
for r, b in mine:
    ep = r.get("episodes")
    if r.get("kind") == "episode" and ep: eps[ep[0]].append((r, b))
    elif r.get("kind") == "batch" or (ep and ep[1] > ep[0]): packs.append((r, b))
    else: eps["?"].append((r, b))
n = ent[target]["eps"] or max([k for k in eps if k != "?"] or [0])
print(f"\nepisode -> sources (best seeded first; p = model confidence)")
for e in range(1, n + 1):
    lst = sorted(eps.get(e, []), key=lambda x: -x[0]["seeders"])
    if lst:
        r, b = lst[0]; print(f"  E{e:02d}  {len(lst):>2} sources  best: {r['title'][:78]}  [{r['seeders']} seeds, {r['size']}, p={b:.3f}]")
    else: print(f"  E{e:02d}   0 sources")
print(f"\nbatches / packs for this entry: {len(packs)}")
for r, b in sorted(packs, key=lambda x: -x[0]["seeders"])[:12]:
    print(f"  eps {r.get('episodes')}  {r['title'][:80]}  [{r['seeders']} seeds, {r['size']}, p={b:.3f}]")
print(f"unnumbered (movie/special/unknown kind): {len(eps.get('?', []))}")
if args.out:
    with open(args.out, "w", encoding="utf8") as f:
        f.write("assigned\tprob\tkind\tepisodes\tseeders\tsize\tpublished\ttitle\n")
        for r, t, b, top in out:
            f.write(f"{ent[t]['label'] if t else ''}\t{b:.3f}\t{r.get('kind')}\t{r.get('episodes')}\t{r['seeders']}\t{r['size']}\t{r['published']}\t{r['title']}\n")
