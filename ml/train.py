"""L1 prototype: XGBoost release -> AniDB-entry matcher on trusted Tosho rows.

usage: train.py <data_dir> <entries.json> [--cutoff 2025-01-01] [--topk 8] [--save model.json]
  <data_dir>: rows.jsonl + parsed.jsonl (export_rows.py + release-parse ml_parse) and, for the
  hardening run, extra.jsonl + extra_parsed.jsonl (export_extra.py). <entries.json>: fetch_anilist.py.
The candidate pool is EVERY entry the dataset knows (trusted or not). Training adds "no match" groups
(true entry removed); evaluation covers present / removed / non-anime releases, slices, and scores the
extra sets (Jujutsu Kaisen, Mushoku crawl). Scratch, not shipped.
"""
import argparse, json, re, random, datetime as dt, zlib, collections
import numpy as np
import xgboost as xgb
from rapidfuzz import fuzz
from sklearn.feature_extraction.text import TfidfVectorizer
from sklearn.metrics import roc_auc_score

ap = argparse.ArgumentParser()
ap.add_argument("data"); ap.add_argument("entries")
ap.add_argument("--cutoff", default="2025-01-01"); ap.add_argument("--topk", type=int, default=8)
ap.add_argument("--holdout-pct", type=int, default=10); ap.add_argument("--nomatch-pct", type=int, default=8)
ap.add_argument("--save", default=""); ap.add_argument("--out", default="")
args = ap.parse_args()
random.seed(7)

def norm(s):
    s = (s or "").lower()
    s = re.sub(r"[^0-9a-z぀-ヿ一-鿿Ѐ-ӿ]+", " ", s)
    return re.sub(r"\s+", " ", s).strip()

def day(d, default=None):
    try: return dt.date(d["year"], d["month"] or 6, d["day"] or 15).toordinal() if d and d.get("year") else default
    except Exception: return default

# ---- entries (every dataset entry with AniList metadata) ------------------
E = json.load(open(args.entries, encoding="utf8"))
a2l = {int(k): v for k, v in E["a2l"].items()}
fr = {int(k): v for k, v in E["fribb"].items()}
al = {int(k): v for k, v in E["anilist"].items()}
FORMATS = ["TV", "TV_SHORT", "MOVIE", "SPECIAL", "OVA", "ONA", "MUSIC"]
ent = {}
for aid, lid in a2l.items():
    m = al.get(lid)
    if not m: continue
    names = {norm(x) for x in [m["title"].get("romaji"), m["title"].get("english"), m["title"].get("native"), *(m.get("synonyms") or [])] if x}
    names.discard("")
    if not names: continue
    ent[aid] = dict(names=sorted(names), tvdb=fr[aid].get("tvdb"), season=fr[aid].get("season"),
                    fmt=FORMATS.index(m["format"]) if m.get("format") in FORMATS else -1,
                    eps=m.get("episodes"), start=day(m.get("startDate")), end=day(m.get("endDate")),
                    label=m["title"].get("romaji") or m["title"].get("english") or "?")
aids = sorted(ent)
by_tvdb = {}
for a in aids:
    if ent[a]["tvdb"]: by_tvdb.setdefault(ent[a]["tvdb"], []).append(a)
def siblings(a):
    t = ent[a]["tvdb"]; return [x for x in by_tvdb.get(t, []) if x != a] if t else []
def holdout(a): return zlib.crc32(str(a).encode()) % 100 < args.holdout_pct
print(f"entry pool: {len(ent)}; held out: {sum(holdout(a) for a in aids)}")

# ---- releases ------------------------------------------------------------
def load2(rows_f, parsed_f):
    out = []
    with open(rows_f, encoding="utf8") as f1, open(parsed_f, encoding="utf8") as f2:
        for l1, l2 in zip(f1, f2):
            r = json.loads(l1); p = json.loads(l2)
            r["show"] = norm(p.get("title") or r["title"])
            r.update({k: v for k, v in p.items() if k != "title"})
            r["day"] = dt.date.fromisoformat(r["published"][:10]).toordinal()
            out.append(r)
    return out

rows = [r for r in load2(f"{args.data}/rows.jsonl", f"{args.data}/parsed.jsonl") if r["aid"] in ent]
extra = load2(f"{args.data}/extra.jsonl", f"{args.data}/extra_parsed.jsonl")
print("trusted releases with an entry:", len(rows), " extra rows:", collections.Counter(r["tag"] for r in extra))

# ---- candidate retrieval -------------------------------------------------
name_list, name_aid = [], []
for a in aids:
    for n in ent[a]["names"]: name_list.append(n); name_aid.append(a)
name_aid = np.array(name_aid)
tf = TfidfVectorizer(analyzer="char_wb", ngram_range=(3, 4), min_df=1, sublinear_tf=True, dtype=np.float32)
NM = tf.fit_transform(name_list)
shows = sorted({r["show"] for r in rows + extra})
cand_of, tfidf_of = {}, {}
for i in range(0, len(shows), 2000):
    chunk = shows[i:i + 2000]
    S = (tf.transform(chunk) @ NM.T).tocsr()
    for j, s in enumerate(chunk):
        row = S.getrow(j)
        if row.nnz == 0: cand_of[s] = []; continue
        idx = row.indices[np.argsort(-row.data)[:60]]
        seen, out = set(), []
        for k in idx:
            a = int(name_aid[k])
            if a not in seen: seen.add(a); out.append(a)
            if len(out) >= args.topk: break
        cand_of[s] = out
        for k in idx: tfidf_of.setdefault((s, int(name_aid[k])), float(row[0, k]))
print("unique show strings:", len(shows))

# ---- features ------------------------------------------------------------
FEATS = ["ts", "r", "pr", "tfidf", "sim_rank", "sim_gap", "n_cand", "n_sib", "rel_has_season", "rel_season", "ent_season",
         "season_match", "season_diff", "ent_s1", "d_start", "d_end", "in_window", "ep_first", "ep_last", "ent_eps",
         "ep_fits", "ep_over", "is_batch", "rel_kind", "rel_extras", "ent_fmt", "movie_match", "n_names", "best_ts_all"]
KIND = {"episode": 0, "batch": 1, "movie": 2, "special": 3, "unknown": 4}
nan = float("nan"); fuzz_cache = {}
def fz(show, a):
    k = (show, a)
    if k not in fuzz_cache:
        ns = ent[a]["names"]
        fuzz_cache[k] = (max(fuzz.token_set_ratio(show, n) for n in ns), max(fuzz.ratio(show, n) for n in ns),
                         max(fuzz.partial_ratio(show, n) for n in ns))
    return fuzz_cache[k]

def build(r, force_true, drop_true, exclude_held):
    cands = list(cand_of.get(r["show"], []))
    retrieved = r.get("aid") in cands
    for a in list(cands): cands += [s for s in siblings(a) if s not in cands]
    true = r.get("aid") if r.get("aid") in ent else None
    if drop_true and true in cands: cands.remove(true)
    elif force_true and true and true not in cands: cands.append(true)
    if exclude_held: cands = [a for a in cands if not holdout(a)]
    sims = {a: fz(r["show"], a) for a in cands}
    order = sorted(cands, key=lambda a: -sims[a][0])
    best = sims[order[0]][0] if order else 0
    X, y, ids = [], [], []
    ep = r.get("episodes"); seasons = r.get("seasons") or []
    for a in cands:
        e = ent[a]; ts, rt, pr = sims[a]
        es = e["season"]; rs = seasons[0] if seasons else nan
        sm = 1.0 if (es is not None and es in seasons) else (0.0 if seasons and es is not None else nan)
        ef, el = (ep[0], ep[1]) if ep else (nan, nan)
        eps = e["eps"] if e["eps"] else nan
        X.append([ts, rt, pr, tfidf_of.get((r["show"], a), 0.0), order.index(a), best - ts, len(cands), len(by_tvdb.get(e["tvdb"], [])) if e["tvdb"] else 0,
                  1.0 if seasons else 0.0, rs, es if es is not None else nan, sm,
                  (rs - es) if (seasons and es is not None) else nan, 1.0 if es == 1 else 0.0,
                  (r["day"] - e["start"]) if e["start"] else nan, (r["day"] - e["end"]) if e["end"] else nan,
                  1.0 if e["start"] and r["day"] >= e["start"] - 21 and (not e["end"] or r["day"] <= e["end"] + 400) else 0.0,
                  ef, el, eps, (1.0 if el <= eps else 0.0) if (ep and eps == eps) else nan, (el - eps) if (ep and eps == eps) else nan,
                  1.0 if r.get("kind") == "batch" else 0.0, KIND.get(r.get("kind"), 4), 1.0 if r.get("extras") else 0.0,
                  e["fmt"], 1.0 if (r.get("kind") == "movie") == (e["fmt"] == 2) else 0.0, len(e["names"]), best])
        y.append(1 if a == true else 0); ids.append(a)
    return X, y, ids, retrieved

def make(rs, force_true=True, drop=None, exclude_held=False):
    X, y, grp, ids, ret = [], [], [], [], []
    for gi, r in enumerate(rs):
        x, yy, ii, rv = build(r, force_true, bool(drop and drop[gi]), exclude_held)
        X += x; y += yy; ids += ii; grp += [gi] * len(x); ret.append(rv)
    return (np.array(X, dtype=np.float32).reshape(-1, len(FEATS)), np.array(y), np.array(grp), np.array(ids), np.array(ret))

cut = dt.date.fromisoformat(args.cutoff).toordinal(); val_from = cut - 120
train_r = [r for r in rows if r["day"] < val_from and not holdout(r["aid"])]
val_r = [r for r in rows if val_from <= r["day"] < cut and not holdout(r["aid"])]
test_r = [r for r in rows if r["day"] >= cut]
test_new = [r for r in test_r if holdout(r["aid"])]
print(f"train {len(train_r)}  val {len(val_r)}  test {len(test_r)}  (held-out entries: {len(test_new)})")

nomatch = lambda n: [random.random() < args.nomatch_pct / 100 for _ in range(n)]
Xtr, ytr, gtr, _, _ = make(train_r, True, nomatch(len(train_r)), True)
Xva, yva, gva, _, _ = make(val_r, True, nomatch(len(val_r)), True)
print("pairs train", Xtr.shape, "pos rate", ytr.mean().round(4))
dtr = xgb.DMatrix(Xtr, ytr, feature_names=FEATS); dva = xgb.DMatrix(Xva, yva, feature_names=FEATS)
params = dict(objective="binary:logistic", eval_metric="aucpr", max_depth=6, eta=0.08, subsample=0.8, colsample_bytree=0.8,
              min_child_weight=3, tree_method="hist", nthread=8)
bst = xgb.train(params, dtr, 600, evals=[(dva, "val")], early_stopping_rounds=30, verbose_eval=100)
if args.save: bst.save_model(args.save)

def predict(rs, drop=None, force_true=True):
    X, y, g, ids, ret = make(rs, force_true, drop)
    p = bst.predict(xgb.DMatrix(X, feature_names=FEATS)) if len(X) else np.zeros(0)
    res = []
    starts = np.r_[0, np.flatnonzero(np.diff(g)) + 1, len(g)] if len(g) else np.array([0])
    gi_to = {int(g[starts[k]]): k for k in range(len(starts) - 1)}
    for gi in range(len(rs)):
        if gi not in gi_to: res.append((0.0, 0.0, None, None, False)); continue
        k = gi_to[gi]; s, e = starts[k], starts[k + 1]; pp = p[s:e]; o = np.argsort(-pp)
        top = int(ids[s:e][o[0]])
        res.append((float(pp[o[0]]), float(pp[o[0]] - (pp[o[1]] if len(o) > 1 else 0)), top, float(X[s + o[0], FEATS.index("in_window")]), bool(ret[gi])))
    return res, X, y, p

def answered(res, t, mg=0.3): return [(b >= t and m >= mg) for b, m, *_ in res]

# ---- 1. headline: present / true entry removed / non-anime ----------------
res, X, y, p = predict(test_r)
corr = np.array([top == r["aid"] for (b, m, top, w, rt), r in zip(res, test_r)])
print(f"\n== TEST 2025+ (true entry in pool): {len(test_r)} releases, pair AUC {roc_auc_score(y, p):.4f}, retrieved w/o injection {np.mean([x[4] for x in res]):.3f}")
print(f"   top-1 accuracy {corr.mean():.4f}")
dropflag = [random.random() < 0.10 for _ in test_r]
sub = [r for r, d in zip(test_r, dropflag) if d]
res_d, *_ = predict(sub, [True] * len(sub))
non = [r for r in extra if r["tag"] == "nonanime"]
res_n, *_ = predict(non, [False] * len(non), force_true=False)
print(f"   no-match sets: true entry removed {len(sub)}, non-anime {len(non)}")
print("   thr   | present: cover/prec | removed: wrongly answered | non-anime: wrongly answered | mixed precision (all answered)")
for t in (0.5, 0.8, 0.9, 0.95, 0.98, 0.99):
    a = np.array(answered(res, t)); ad = np.array(answered(res_d, t)); an = np.array(answered(res_n, t))
    right = (a & corr).sum(); tot = a.sum() + ad.sum() + an.sum()
    print(f"   {t:<5} | {a.mean():.3f} / {corr[a].mean() if a.any() else float('nan'):.4f} | {ad.mean():.4f} | {an.mean() if len(an) else float('nan'):.4f} | {right / max(tot, 1):.4f}")

res_h, *_ = predict(test_new)
ch = np.array([top == r["aid"] for (b, m, top, w, rt), r in zip(res_h, test_new)])
print(f"\n== held-out entries: {len(test_new)} releases, top-1 {ch.mean():.4f}; p>=0.95 cover {np.mean(answered(res_h, .95)):.3f} prec {ch[answered(res_h, .95)].mean():.4f}")

# ---- 2. slices at p>=0.95, margin>=0.3 -------------------------------------
gsize = collections.Counter((r.get("group") or "").lower() for r in train_r)
def slc(name, key):
    buckets = collections.defaultdict(list)
    for i, r in enumerate(test_r): buckets[key(i, r)].append(i)
    print(f"\n-- slice: {name} (p>=0.95)")
    a = np.array(answered(res, 0.95))
    for k, ix in sorted(buckets.items(), key=lambda kv: str(kv[0])):
        ix = np.array(ix); aa = a[ix]
        print(f"   {str(k):<24} n={len(ix):<6} top1 {corr[ix].mean():.4f}  coverage {aa.mean():.3f}  precision {corr[ix][aa].mean() if aa.any() else float('nan'):.4f}")
slc("release kind", lambda i, r: r.get("kind"))
slc("group size in training", lambda i, r: "unseen" if gsize[(r.get("group") or "").lower()] == 0 else ("<50" if gsize[(r.get("group") or "").lower()] < 50 else ("50-500" if gsize[(r.get("group") or "").lower()] < 500 else ">=500")))
slc("true entry in air window", lambda i, r: "in window" if res[i][3] == 1.0 else "outside/unknown")
slc("has season siblings", lambda i, r: "sibling seasons" if len(siblings(r["aid"])) else "standalone")
slc("year", lambda i, r: r["published"][:4])

# ---- 3. extra sets ----------------------------------------------------------
def show(tag, ex):
    rs, *_ = predict(ex)
    names = collections.Counter(); amb = []
    for (b, m, top, w, rt), r in zip(rs, ex):
        ok = b >= 0.95 and m >= 0.3
        names[(ent[top]["label"] + f" [{top}]") if (ok and top) else "(abstain)"] += 1
        if not ok: amb.append((b, r["title"]))
    print(f"\n== {tag}: {len(ex)} releases; answered {sum(v for k, v in names.items() if k != '(abstain)')}")
    for k, v in names.most_common(8): print(f"   {v:>5}  {k}")
    return rs
jj = [r for r in extra if r["tag"] == "jjk_unmapped"]; jg = [r for r in extra if r["tag"] == "jjk_guess"]
rj = show("Jujutsu Kaisen, releases with no aid", jj)
rg = show("Jujutsu Kaisen, Tosho guess rows (compare with the guess)", jg)
agree = sum(1 for (b, m, top, w, rt), r in zip(rg, jg) if top == r["aid"] and b >= 0.95 and m >= 0.3)
dis = [(r, ent[top]["label"], top) for (b, m, top, w, rt), r in zip(rg, jg) if top != r["aid"] and b >= 0.95 and m >= 0.3]
print(f"   model agrees with the guess on {agree}; answers differently on {len(dis)}; abstains on {len(jg) - agree - len(dis)}")
for r, lab, top in dis[:6]: print(f"      guess {r['aid']} model {top} ({lab}) | {r['title'][:90]}")
mu = [r for r in extra if r["tag"] == "mushoku"]
rm = show("Mushoku Tensei crawl (1,000)", mu)
if args.out:
    import csv
    with open(args.out, "w", encoding="utf8", newline="") as f:
        w = csv.writer(f, delimiter="\t"); w.writerow(["tag", "id", "published", "prob", "margin", "top_aid", "top_entry", "title"])
        for tag, ex, rs_ in (("jjk_unmapped", jj, rj), ("jjk_guess", jg, rg), ("mushoku", mu, rm)):
            for r, (b, m, top, wv, rt) in zip(ex, rs_):
                w.writerow([tag, r["id"], r["published"], round(b, 3), round(m, 3), top, ent[top]["label"] if top else "", r["title"]])
imp = bst.get_score(importance_type="gain")
print("\ntop features by gain:", sorted(((round(v, 1), k) for k, v in imp.items()), reverse=True)[:10])
