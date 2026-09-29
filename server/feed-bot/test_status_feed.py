"""Tests for status_feed.py, with made-up API data. Run: python3 -m unittest test_status_feed"""

import json
import tempfile
import unittest
from pathlib import Path

import status_feed as sf

PUBLIC = sf.PUBLIC_GROUP
PRIVATE = "00000000-aaaa-bbbb-cccc-000000000001"


def server(region, country, groups):
    return {"sid": "0", "oper": "x", "group_ids": groups, "default_region": region,
            "region": region.split("-")[-1], "country_code": country}


def match(mode, lobby_type, players, player_limit, broadcaster, spectators=0):
    people = [{"user_id": f"u-{mode}-{lobby_type}-{i}", "team": "blue"} for i in range(players)]
    people += [{"user_id": f"s-{mode}-{i}", "team": "spectator"} for i in range(spectators)]
    return {"mode": mode, "lobby_type": lobby_type, "player_count": players, "player_limit": player_limit,
            "limit": player_limit * 2, "players": people, "broadcaster": broadcaster}


US = server("us-illinois", "US", [PUBLIC])
GB = server("gb-england", "GB", [PUBLIC, PRIVATE])
AU = server("au-queensland", "AU", [PRIVATE])
ODD = server("xx-nowhere", "ZZ", [PUBLIC])

SAMPLE = {
    "uptime_mins": 60,
    "update_time": "2026-09-29T12:00:00Z",
    "player_count": 12,
    "gameservers": [US, US, GB, AU, ODD],
    "labels": [
        match("social_2.0", "public", 7, 12, US),
        match("echo_arena", "public", 4, 8, GB, spectators=1),
        match("echo_arena", "private", 1, 8, AU),
    ],
}
T = sf.parse_time("2026-09-29T12:00:10Z")


class AggregateTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.history = sf.History(Path(self.tmp.name))

    def tearDown(self):
        self.tmp.cleanup()

    def test_numbers(self):
        out = sf.aggregate(SAMPLE, self.history, T)
        self.assertEqual(out["status"], "ok")
        self.assertEqual(out["started_at"], "2026-09-29T11:00:00+00:00")
        self.assertEqual(out["servers"], {"total": 5, "public": 4, "private": 1})
        # 3 matches on 5 servers; 2 on the 4 public ones; 1 on the 1 private one.
        self.assertEqual(out["usage"], {"total": 60, "public": 50, "private": 100})
        self.assertEqual(out["regions"]["NA"], {"servers": 2, "usage": 50})
        self.assertEqual(out["regions"]["EU"], {"servers": 1, "usage": 100})
        self.assertEqual(out["regions"]["OCE"], {"servers": 1, "usage": 100})
        self.assertEqual(out["modes"]["lobby"]["public"], {"matches": 1, "players": 7, "limit": 12, "spectators": 0})
        self.assertEqual(out["modes"]["arena"]["public"]["spectators"], 1)
        self.assertEqual(out["modes"]["arena"]["private"]["players"], 1)
        self.assertEqual(out["modes"]["combat"]["public"]["matches"], 0)
        self.assertEqual(out["players"]["online"], 12)
        self.assertEqual(out["players"]["last_hour"], 13)
        # Unknown region with an unknown country: left off the map.
        self.assertEqual({l["region"] for l in out["locations"]}, {"us-illinois", "gb-england", "au-queensland"})
        us = next(l for l in out["locations"] if l["region"] == "us-illinois")
        self.assertEqual((us["servers"], us["matches"]), (2, 1))

    def test_publishes_no_player_data(self):
        text = json.dumps(sf.aggregate(SAMPLE, self.history, T))
        self.assertNotIn("u-social", text)
        self.assertNotIn("user_id", text)

    def test_history_windows_and_pseudonyms(self):
        self.history.record({"a", "b"}, T - 2 * 86400)
        self.history.record({"b"}, T - 1800)
        self.history.record({"c"}, T - 31 * 86400)  # already too old
        self.history.record(set(), T)
        self.assertEqual(self.history.counts(T), {"last_hour": 1, "last_24h": 1, "last_30d": 2})
        self.history.save()
        stored = (Path(self.tmp.name) / "history.json").read_text()
        self.assertNotIn('"a"', stored)
        self.assertEqual(len(json.loads(stored)["seen"]), 2)
        again = sf.History(Path(self.tmp.name))
        self.assertEqual(again.counts(T), {"last_hour": 1, "last_24h": 1, "last_30d": 2})

    def test_forget(self):
        self.history.record({"a", "b"}, T)
        self.history.forget("a")
        self.history.record({"a"}, T + 30)
        self.assertEqual(self.history.counts(T + 30)["last_hour"], 1)
        self.assertNotIn(self.history.pseudonym("a"), self.history.seen)

    def test_stale_source(self):
        out = sf.aggregate(SAMPLE, self.history, T + 3600)
        self.assertEqual(out["status"], "stale")


if __name__ == "__main__":
    unittest.main()
