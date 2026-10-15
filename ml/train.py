"""L1 prototype: XGBoost release -> AniDB-entry matcher on trusted Tosho rows.

usage: train.py <data_dir> <entries.json> [--cutoff 2025-01-01] [--topk 8]
  <data_dir> holds rows.jsonl + parsed.jsonl (export_rows.py + release-parse ml_parse),
  <entries.json> is fetch_anilist.py's output. Prints pair AUC and release-level
  precision/coverage (time split + held-out entries). Scratch, not shipped.
"""
import argparse, json, re, sys, datetime as dt, zlib
import numpy as np
import xgboost as xgb
from rapidfuzz import fuzz
from sklearn.feature_extraction.text import TfidfVectorizer
from sklearn.metrics import roc_auc_score

ap = argparse.ArgumentParser()
ap.add_argument("data"); ap.add_argument("entries")
ap.add_argument("--cutoff", default="2025-01-01"); ap.add_argument("--topk", type=int, default=8)
ap.add_argument("--holdout-pct", type=int, default=10)
ap.add_argument("--save", default="")
args = ap.parse_args()

def norm(s):
    s = (s or "").lower()
    s = re.sub(r"[^0-9a-z぀-ヿ一-鿿Ѐ-ӿ]+", " ", s)
    return re.sub(r"\s+", " ", s).strip()

def day(d, default=None):
    try: return dt.date(d["year"], d["month"] or 6, d["day"] or 15).toordinal() if d and d.get("year") else default
    except Exception: return default

# ---- entries -------------------------------------------------------------
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
                    eps=m.get("episodes"), start=day(m.get("startDate")), end=day(m.get("endDate")))
aids = sorted(ent)
by_tvdb = {}
for a in aids:
    if ent[a]["tvdb"]: by_tvdb.setdefault(ent[a]["tvdb"], []).append(a)
def siblings(a):
    t = ent[a]["tvdb"]; return [x for x in by_tvdb.get(t, []) if x != a] if t else []
def holdout(a): return zlib.crc32(str(a).encode()) % 100 < args.holdout_pct
print(f"entries with metadata: {len(ent)}; held out: {sum(holdout(a) for a in aids)}")

# ---- releases ------------------------------------------------------------
rows = []
with open(f"{args.data}/rows.jsonl", encoding="utf8") as f1, open(f"{args.data}/parsed.jsonl", encoding="utf8") as f2:
    for l1, l2 in zip(f1, f2):
        r = json.loads(l1); p = json.loads(l2)
        if r["aid"] not in ent: continue
        r.update(p); r["day"] = dt.date.fromisoformat(r["published"][:10]).toordinal()
        r["show"] = norm(p.get("title") or r["title"])
        rows.append(r)
print("releases with a mapped entry:", len(rows))

# ---- candidate retrieval (char n-gram tf-idf over entry names) -----------
name_list, name_aid = [], []
for a in aids:
    for n in ent[a]["names"]: name_list.append(n); name_aid.append(a)
name_aid = np.array(name_aid)
tf = TfidfVectorizer(analyzer="char_wb", ngram_range=(3, 4), min_df=1, sublinear_tf=True, dtype=np.float32)
NM = tf.fit_transform(name_list)
shows = sorted({r["show"] for r in rows})
cand_of, tfidf_of = {}, {}
CH = 2000
for i in range(0, len(shows), CH):
    chunk = shows[i:i + CH]
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
        for k in idx:
            tfidf_of.setdefault((s, int(name_aid[k])), float(row[0, k]))
print("unique show strings:", len(shows))

# ---- pair features -------------------------------------------------------
FEATS = ["ts", "r", "pr", "tfidf", "sim_rank", "sim_gap", "n_cand", "n_sib", "rel_has_season", "rel_season", "ent_season",
         "season_match", "season_diff", "ent_s1", "d_start", "d_end", "in_window", "ep_first", "ep_last", "ent_eps",
         "ep_fits", "ep_over", "is_batch", "rel_kind", "rel_extras", "ent_fmt", "movie_match", "n_names"]
KIND = {"episode": 0, "batch": 1, "movie": 2, "special": 3, "unknown": 4}
nan = float("nan")
fuzz_cache = {}
def fz(show, a):
    k = (show, a)
    if k not in fuzz_cache:
        ns = ent[a]["names"]
        fuzz_cache[k] = (max(fuzz.token_set_ratio(show, n) for n in ns), max(fuzz.ratio(show, n) for n in ns),
                         max(fuzz.partial_ratio(show, n) for n in ns))
    return fuzz_cache[k]

def build(r, force_true, exclude_held):
    cands = list(cand_of.get(r["show"], []))
    retrieved = r["aid"] in cands
    for a in list(cands): cands += [s for s in siblings(a) if s not in cands]
    if force_true and r["aid"] not in cands: cands.append(r["aid"])
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
                  e["fmt"], 1.0 if (r.get("kind") == "movie") == (e["fmt"] == 2) else 0.0, len(e["names"])])
        y.append(1 if a == r["aid"] else 0); ids.append(a)
    return X, y, ids, retrieved

def make(rs, force_true, exclude_held):
    X, y, grp, ids, ret = [], [], [], [], []
    for gi, r in enumerate(rs):
        x, yy, ii, rv = build(r, force_true, exclude_held)
        X += x; y += yy; ids += ii; grp += [gi] * len(x); ret.append(rv)
    return np.array(X, dtype=np.float32), np.array(y), np.array(grp), np.array(ids), np.array(ret)

cut = dt.date.fromisoformat(args.cutoff).toordinal()
val_from = cut - 120
train_r = [r for r in rows if r["day"] < val_from and not holdout(r["aid"])]
val_r = [r for r in rows if val_from <= r["day"] < cut and not holdout(r["aid"])]
test_r = [r for r in rows if r["day"] >= cut]
test_new = [r for r in test_r if holdout(r["aid"])]
print(f"train {len(train_r)}  val {len(val_r)}  test {len(test_r)}  (test on held-out entries: {len(test_new)})")

Xtr, ytr, gtr, _, _ = make(train_r, True, True)
Xva, yva, gva, _, _ = make(val_r, True, True)
print("pairs train", Xtr.shape, "pos rate", ytr.mean().round(3))
dtr = xgb.DMatrix(Xtr, ytr, feature_names=FEATS); dva = xgb.DMatrix(Xva, yva, feature_names=FEATS)
params = dict(objective="binary:logistic", eval_metric="aucpr", max_depth=6, eta=0.08, subsample=0.8, colsample_bytree=0.8,
              min_child_weight=3, tree_method="hist", nthread=8)
bst = xgb.train(params, dtr, 600, evals=[(dva, "val")], early_stopping_rounds=30, verbose_eval=50)
if args.save: bst.save_model(args.save)

def evaluate(name, rs, force_true):
    X, y, g, ids, ret = make(rs, force_true, False)
    p = bst.predict(xgb.DMatrix(X, feature_names=FEATS))
    print(f"\n== {name}: {len(rs)} releases, {len(y)} pairs, AUC {roc_auc_score(y, p):.4f}, true entry retrieved w/o injection: {ret.mean():.3f}")
    best_p = np.zeros(len(rs)); correct = np.zeros(len(rs), bool); margin = np.zeros(len(rs))
    sim_correct = np.zeros(len(rs), bool)
    starts = np.r_[0, np.flatnonzero(np.diff(g)) + 1, len(g)]
    for k in range(len(starts) - 1):
        s, e = starts[k], starts[k + 1]; pp = p[s:e]; o = np.argsort(-pp)
        best_p[k] = pp[o[0]]; margin[k] = pp[o[0]] - (pp[o[1]] if len(o) > 1 else 0); correct[k] = y[s:e][o[0]] == 1
        sim_correct[k] = y[s:e][np.argmax(X[s:e, 0] - 0.001 * X[s:e, 4])] == 1
    print(f"top-1 accuracy: model {correct.mean():.4f}  title-similarity-only {sim_correct.mean():.4f}")
    for t in (0.5, 0.8, 0.9, 0.95, 0.98, 0.99):
        m = (best_p >= t) & (margin >= 0.3)
        print(f"  p>={t:<4} margin>=0.3: coverage {m.mean():.3f}  precision {correct[m].mean() if m.any() else float('nan'):.4f}")
    by_y = {}
    for r, c_, bp in zip(rs, correct, best_p):
        yr = r["published"][:4]; v = by_y.setdefault(yr, [0, 0]); v[0] += 1; v[1] += c_
    print("  accuracy by year:", {k: round(v[1] / v[0], 4) for k, v in sorted(by_y.items())})
    return X, y, g, p

evaluate("TEST (2025+, all entries)", test_r, True)
evaluate("TEST on HELD-OUT entries (never seen in training)", test_new, True)
imp = bst.get_score(importance_type="gain")
print("\ntop features by gain:", sorted(((round(v, 1), k) for k, v in imp.items()), reverse=True)[:12])
