"""Fetch AniList metadata for every AniDB entry that has trusted Tosho releases.
usage: fetch_anilist.py <dataset.db> <fribb.json> <out.json>"""
import json, sqlite3, sys, time, urllib.request
db, fribb, out = sys.argv[1:4]
c = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
aids = {r[0] for r in c.execute("select distinct aid from release_aid")}  # every entry the dataset knows, trusted or not
fr = {e["anidb_id"]: e for e in json.load(open(fribb, encoding="utf8")) if "anidb_id" in e}
a2l = {a: fr[a]["anilist_id"] for a in aids if a in fr and fr[a].get("anilist_id")}
print(len(aids), "trusted aids,", len(a2l), "mapped to AniList")
Q = """query($ids:[Int]){Page(perPage:50){media(id_in:$ids,type:ANIME){id format episodes seasonYear season
 startDate{year month day} endDate{year month day} title{romaji english native} synonyms}}}"""
import os
prev = json.load(open(out, encoding="utf8"))["anilist"] if os.path.exists(out) else {}
res = {int(k): v for k, v in prev.items()}
ids = sorted(set(a2l.values()) - set(res))
for i in range(0, len(ids), 50):
    body = json.dumps({"query": Q, "variables": {"ids": ids[i:i+50]}}).encode()
    for attempt in range(5):
        try:
            r = urllib.request.urlopen(urllib.request.Request("https://graphql.anilist.co", body,
                {"Content-Type": "application/json", "Accept": "application/json", "User-Agent": "nyaa-stream-ml/0.1"}), timeout=60)
            for m in json.load(r)["data"]["Page"]["media"]: res[m["id"]] = m
            break
        except Exception as e:
            print("retry", i, e); time.sleep(10 * (attempt + 1))
    print("batch", i // 50 + 1, "of", (len(ids) + 49) // 50, "have", len(res), flush=True); time.sleep(1.2)
json.dump({"a2l": a2l, "fribb": {a: {"tvdb": fr[a].get("tvdb_id"), "season": (fr[a].get("season") or {}).get("tvdb"), "type": fr[a].get("type")} for a in a2l}, "anilist": res}, open(out, "w", encoding="utf8"))
print("fetched", len(res), "of", len(ids))
