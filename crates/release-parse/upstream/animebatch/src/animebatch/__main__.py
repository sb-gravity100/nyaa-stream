import argparse, json, logging, sys
from . import parse_many, search


def main():
    ap = argparse.ArgumentParser(prog="animebatch", description="Filter release titles by anime + season")
    ap.add_argument("file", help="JSON: nyaa crawl ({'items':[{'title':..}]}), list of dicts, or list of strings")
    ap.add_argument("--title", required=True)
    ap.add_argument("--season", type=int)
    ap.add_argument("--episode", type=int)
    ap.add_argument("--no-complete", action="store_true", help="exclude complete-series packs")
    ap.add_argument("--unknown", default="s1", choices=["s1", "include", "exclude"])
    ap.add_argument("--batches", default="include", choices=["include", "only", "exclude"])
    ap.add_argument("--threshold", type=float, default=88)
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--debug", action="store_true", help="log rule traces to animebatch.log")
    a = ap.parse_args()
    if a.debug:
        logging.basicConfig(filename="animebatch.log", level=logging.DEBUG, filemode="w")
    d = json.load(open(a.file, encoding="utf-8"))
    items = d["items"] if isinstance(d, dict) else d
    titles = [i["title"] if isinstance(i, dict) else i for i in items]
    res = search(parse_many(titles), a.title, a.season, a.episode, include_complete=not a.no_complete,
                 unknown=a.unknown, threshold=a.threshold, batches=a.batches)
    if a.json:
        json.dump([{"score": m.score, "reason": m.reason, **m.release.to_dict()} for m in res],
                  sys.stdout, ensure_ascii=False, indent=1)
        return
    for m in res:
        r = m.release
        print(f"{m.score:5.1f} {m.reason:13} {r.batch_kind or 'single':13} S{r.seasons} ep{r.ep_start}-{r.ep_end}  {r.raw[:90]}")
    print(f"{len(res)} match(es) of {len(titles)}")


main()
