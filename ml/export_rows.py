"""Trusted (approved 1-3) releases since a year -> rows.jsonl + titles.txt (same order).
usage: export_rows.py <dataset.db> <out_dir> [from_year=2021]"""
import json, sqlite3, sys, os
db, out = sys.argv[1], sys.argv[2]; y = sys.argv[3] if len(sys.argv) > 3 else "2021"
os.makedirs(out, exist_ok=True)
c = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
rows = c.execute("""select r.id, r.published, r.title, a.aid from release_aid a join releases r on r.id=a.nyaa_id
 where a.approved between 1 and 3 and substr(r.published,1,4)>=? order by r.id""", (y,)).fetchall()
with open(f"{out}/rows.jsonl", "w", encoding="utf8") as f, open(f"{out}/titles.txt", "w", encoding="utf8") as t:
    for i, p, title, aid in rows:
        title = title.replace("\n", " ").replace("\r", " ")
        f.write(json.dumps({"id": i, "published": p, "title": title, "aid": aid}) + "\n"); t.write(title + "\n")
print(len(rows), "rows")
