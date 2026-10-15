"""Extra evaluation rows -> extra.jsonl + extra_titles.txt (same order).
tags: nonanime (category not 1_x, since 2021), jjk_unmapped (no release_aid row), jjk_guess (approved=0, with the
guess aid), mushoku (a nyaa crawl json, optional).
usage: export_extra.py <dataset.db> <out_dir> [mushoku_crawl.json]"""
import json, sqlite3, sys
db, out = sys.argv[1], sys.argv[2]
c = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
rows = []
def add(tag, i, pub, title, aid=None, **kw):
    rows.append({"tag": tag, "id": i, "published": pub, "title": title.replace("\n", " ").replace("\r", " "), "aid": aid, **kw})
for i, p, t in c.execute("select id,published,title from releases where substr(category,1,2) != '1_' and substr(published,1,4)>='2021'"):
    add("nonanime", i, p, t)
JJK = "(lower(r.title) like '%jujutsu%' or lower(r.title) like '%呪術%')"
for i, p, t in c.execute(f"select r.id,r.published,r.title from releases r left join release_aid a on a.nyaa_id=r.id where {JJK} and a.nyaa_id is null"):
    add("jjk_unmapped", i, p, t)
for i, p, t, aid in c.execute(f"select r.id,r.published,r.title,a.aid from releases r join release_aid a on a.nyaa_id=r.id where {JJK} and a.approved=0"):
    add("jjk_guess", i, p, t, aid)
if len(sys.argv) > 3:
    for it in json.load(open(sys.argv[3], encoding="utf8"))["items"]:
        add("mushoku", it["id"], it["date"], it["title"])
with open(f"{out}/extra.jsonl", "w", encoding="utf8") as f, open(f"{out}/extra_titles.txt", "w", encoding="utf8") as t:
    for r in rows: f.write(json.dumps(r) + "\n"); t.write(r["title"] + "\n")
from collections import Counter
print(Counter(r["tag"] for r in rows))
