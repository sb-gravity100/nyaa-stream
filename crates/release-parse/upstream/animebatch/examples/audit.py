"""Run animebatch over crawl JSON files; write audit.log; print stats. Usage: python audit.py a.json [b.json ...]"""
import sys, json, re, logging, collections, random
sys.path.insert(0, "../src")
from animebatch import parse
logging.basicConfig(filename="audit.log", level=logging.DEBUG, filemode="w", format="%(message)s")
SXE = re.compile(r"\bS(\d{1,2})E(\d{1,4})\b")
for path in sys.argv[1:]:
    titles = [i["title"] for i in json.load(open(path, encoding="utf-8"))["items"]]
    rs = [parse(t) for t in titles]
    kinds = collections.Counter(r.batch_kind or "single/none" for r in rs)
    print(f"\n== {path}: {len(rs)} titles\n", dict(kinds.most_common()))
    gt = [(r, SXE.search(r.raw)) for r in rs if SXE.search(r.raw)]
    ok = sum(r.seasons == [int(m[1])] and r.ep_start == r.ep_end == int(m[2]) for r, m in gt)
    print(f"explicit SxxEyy regression: {ok}/{len(gt)} exact; false batches: {sum(r.is_batch for r, _ in gt)}")
    noep = [r for r in rs if r.ep_start is None and not r.is_batch]
    print(f"no episode & not batch (review): {len(noep)}")
    for r in noep[:6]: print("   ", r.raw[:110])
    for name in logging.Logger.manager.loggerDict: pass
    for r in rs: logging.debug("%s | %s | S%s ep%s-%s | %s", r.batch_kind, r.confidence, r.seasons, r.ep_start, r.ep_end, r.raw)
    low = [r for r in rs if r.confidence < 0.5]
    print(f"low confidence (<0.5): {len(low)}")
    random.seed(7)
    for k in ("episode_range", "season_pack", "multi_season", "complete", "box_set", "batch_tag", "disc_pack"):
        pool = [r for r in rs if r.batch_kind == k]
        for r in random.sample(pool, min(3, len(pool))):
            print(f"  [{k}] S{r.seasons} ep{r.ep_start}-{r.ep_end} <- {r.raw[:95]}")
