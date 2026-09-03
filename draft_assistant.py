#!/usr/bin/env python3
"""Live Sleeper fantasy football draft assistant.

Polls the Sleeper API during the draft, tracks every pick in real time, and
always shows the current best pick + backup so slot 4 is never caught by the
60-second pick clock.

Usage:
    python3 draft_assistant.py                 # live mode, polls every 20s
    python3 draft_assistant.py --simulate 20   # pretend top 20 board picks are
                                               # gone, render dashboard once
    python3 draft_assistant.py --once          # single live render, then exit

stdlib + requests only.
"""

import argparse
import json
import os
import re
import subprocess
import sys
import time
from datetime import datetime

import requests

# ---------------------------------------------------------------------------
# Hard facts (do not change)
# ---------------------------------------------------------------------------
USERNAME = "brannflakes"
USER_ID = "560186421480095744"
LEAGUE_ID = "1389740583450316800"
DRAFT_ID = "1389740583450316801"
MY_SLOT = 4
TEAMS = 12
ROUNDS = 14
TOTAL_PICKS = TEAMS * ROUNDS  # 168, but 14 rounds x 12 teams; my last pick is 165
MY_PICKS = [4, 21, 28, 45, 52, 69, 76, 93, 100, 117, 124, 141, 148, 165]

POLL_SECONDS = 20
PICK_CLOCK_SECONDS = 60

API_BASE = "https://api.sleeper.app/v1"
PLAYERS_CACHE_FILE = "players_cache.json"
PLAYERS_CACHE_MAX_AGE = 24 * 3600  # refresh only if cache >24h old
LOG_FILE = "draft_log.txt"

ALLOWED_TES = {"Trey McBride", "Brock Bowers", "Colston Loveland"}
MAX_TE = 1
MAX_QB = 1
MAX_DEF = 1
QB_ANY_PICK = 69          # no QB recs before this pick, except...
QB_VALUE_FALL_PICK = 45   # ...an elite QB still available at pick 45+
QB_VALUE_FALL_NAMES = {"Jayden Daniels", "Jalen Hurts", "Josh Allen"}
QB_DEADLINE = 100         # must have a QB by pick 100
DEF_WINDOW_START = 93
DEF_DEADLINE = 117        # must have a DEF by pick 117
UPSIDE_PICK_START = 117   # from here, prefer the late-round upside pool
BALANCE_THROUGH_PICK = 52 # keep RB/WR counts within 1 through this pick
BYE_GUARD_WEEK = 11
BYE_GUARD_COUNT = 3       # 3+ players on the week-11 bye -> penalize more
BYE_PENALTY_SPOTS = 10
JEANTY_BOOST_TARGET = "Mike Washington Jr."
JEANTY_BOOST_SPOTS = 15
UPSIDE_BOOST_SPOTS = 15
BALANCE_PENALTY_SPOTS = 10

# ---------------------------------------------------------------------------
# Value board (ordered; rank = value). Format: rank Name POS (TEAM BYE)
# ---------------------------------------------------------------------------
VALUE_BOARD_RAW = """
1 Jahmyr Gibbs RB (DET 6), 2 Bijan Robinson RB (ATL 11), 3 Puka Nacua WR (LAR 11),
4 Ja'Marr Chase WR (CIN 6), 5 Jaxon Smith-Njigba WR (SEA 11), 6 Christian McCaffrey RB (SF 8),
7 Amon-Ra St. Brown WR (DET 6), 8 Jonathan Taylor RB (IND 13), 9 De'Von Achane RB (MIA 6),
10 CeeDee Lamb WR (DAL 14), 11 Justin Jefferson WR (MIN 6), 12 James Cook III RB (BUF 7),
13 Drake London WR (ATL 11), 14 Chase Brown RB (CIN 6), 15 Rashee Rice WR (KC 5),
16 Derrick Henry RB (BAL 13), 17 A.J. Brown WR (NE 11), 18 Saquon Barkley RB (PHI 10),
19 Chris Olave WR (NO 8), 20 George Pickens WR (DAL 14), 21 Nico Collins WR (HOU 8),
22 Kenneth Walker RB (KC 5), 23 Omarion Hampton RB (LAC 7), 24 Zay Flowers WR (BAL 13),
25 Ashton Jeanty RB (LV 13, ankle), 26 Malik Nabers WR (NYG 8), 27 Garrett Wilson WR (NYJ 13),
28 Jeremiyah Love RB (ARI 14), 29 Trey McBride TE (ARI 14), 30 DeVonta Smith WR (PHI 10),
31 Tetairoa McMillan WR (CAR 5), 32 Kyren Williams RB (LAR 11), 33 Breece Hall RB (NYJ 13),
34 Brock Bowers TE (LV 13), 35 Josh Allen QB (BUF 7), 36 Emeka Egbuka WR (TB 10),
37 Tee Higgins WR (CIN 6), 38 Javonte Williams RB (DAL 14), 39 Travis Etienne Jr. RB (NO 8),
40 Ladd McConkey WR (LAC 7), 41 Cam Skattebo RB (NYG 8), 42 Davante Adams WR (LAR 11),
43 Josh Jacobs RB (GB 11), 44 Jaylen Waddle WR (DEN 10), 45 D'Andre Swift RB (CHI 10),
46 Jameson Williams WR (DET 6), 47 Bucky Irving RB (TB 10), 48 DJ Moore WR (BUF 7),
49 Terry McLaurin WR (WAS 7), 50 Quinshon Judkins RB (CLE 11), 51 Drake Maye QB (NE 11),
52 Bhayshul Tuten RB (JAX 7), 53 Luther Burden III WR (CHI 10), 54 Rome Odunze WR (CHI 10),
55 Mike Evans WR (SF 8), 56 Colston Loveland TE (CHI 10), 57 Joe Burrow QB (CIN 6),
58 David Montgomery RB (HOU 8), 59 Lamar Jackson QB (BAL 13), 60 Christian Watson WR (GB 11),
61 Courtland Sutton WR (DEN 10), 62 Rhamondre Stevenson RB (NE 11), 63 Jaylen Warren RB (PIT 9),
64 Parker Washington WR (JAX 7), 65 Marvin Harrison Jr. WR (ARI 14), 66 TreVeyon Henderson RB (NE 11),
67 Dak Prescott QB (DAL 14), 68 DK Metcalf WR (PIT 9), 69 Tony Pollard RB (TEN 9),
70 Brian Thomas Jr. WR (JAX 7), 71 Jayden Daniels QB (WAS 7), 72 Jadarian Price RB (SEA 11),
73 Michael Wilson WR (ARI 14), 74 Rico Dowdle RB (PIT 9), 75 Matthew Stafford QB (LAR 11),
76 Michael Pittman Jr. WR (PIT 9), 77 Jalen Hurts QB (PHI 10), 78 Chris Godwin Jr. WR (TB 10),
79 Carnell Tate WR (TEN 9), 80 Seattle Defense DEF (SEA 11), 81 Chuba Hubbard RB (CAR 5),
82 Wan'Dale Robinson WR (TEN 9), 83 Brock Purdy QB (SF 8), 84 Jakobi Meyers WR (JAX 7),
85 Denver Defense DEF (DEN 10), 86 Caleb Williams QB (CHI 10), 87 J.K. Dobbins RB (DEN 10),
88 Josh Downs WR (IND 13), 89 Kenny Gainwell RB (TB 10), 90 Trevor Lawrence QB (JAX 7),
91 Jayden Reed WR (GB 11), 92 Stefon Diggs WR (WAS 7), 93 Jonathon Brooks RB (CAR 5),
94 Quentin Johnston WR (LAC 7), 95 RJ Harvey RB (DEN 10), 96 Jared Goff QB (DET 6),
97 Patrick Mahomes QB (KC 5), 98 Justin Herbert QB (LAC 7), 99 Aaron Jones Sr. RB (MIN 6),
100 Bo Nix QB (DEN 10), 101 Kyle Monangai RB (CHI 10), 102 Jordan Mason RB (MIN 6),
103 Jacory Croskey-Merritt RB (WAS 7), 104 Mike Washington Jr. RB (LV 13),
105 Romeo Doubs WR (NE 11), 106 Tre' Harris WR (LAC 7), 107 Jaxson Dart QB (NYG 8),
108 Baker Mayfield QB (TB 10), 109 Rachaad White RB (WAS 7), 110 Kyler Murray QB (MIN 6)
"""

UPSIDE_POOL = {
    "Mike Washington Jr.", "RJ Harvey", "TreVeyon Henderson", "Jordan Mason",
    "Jonathon Brooks", "Jacory Croskey-Merritt", "Kyle Monangai", "Stefon Diggs",
    "Romeo Doubs", "Tre' Harris", "Carnell Tate", "Josh Downs", "Jayden Reed",
    "Quentin Johnston",
}

# ---------------------------------------------------------------------------
# Name normalization + board parsing
# ---------------------------------------------------------------------------
_SUFFIXES = {"jr", "sr", "ii", "iii", "iv", "v"}


def normalize_name(name):
    """Case-insensitive match key: strip punctuation and Jr./III-style suffixes."""
    s = name.lower().replace("’", "'")
    s = re.sub(r"[.'\-,]", "", s)
    tokens = [t for t in s.split() if t not in _SUFFIXES]
    return " ".join(tokens)


_BOARD_ENTRY_RE = re.compile(
    r"(\d+)\s+(.+?)\s+(QB|RB|WR|TE|DEF)\s+\((\w+)\s+(\d+)[^)]*\)"
)


def parse_board():
    entries = []
    for m in _BOARD_ENTRY_RE.finditer(VALUE_BOARD_RAW):
        rank, name, pos, team, bye = m.groups()
        entries.append({
            "rank": int(rank),
            "name": name,
            "pos": pos,
            "team": team,
            "bye": int(bye),
            "key": normalize_name(name),
            "note": "ankle" if "ankle" in m.group(0) else "",
        })
    entries.sort(key=lambda e: e["rank"])
    return entries


BOARD = parse_board()
BOARD_BY_KEY = {}
for _e in BOARD:
    BOARD_BY_KEY.setdefault(_e["key"], _e)
# Team DEFs on the board, keyed by Sleeper's DEF player_id (the team code).
BOARD_DEF_BY_TEAM = {e["team"]: e for e in BOARD if e["pos"] == "DEF"}
TEAM_BYES = {}
for _e in BOARD:
    TEAM_BYES.setdefault(_e["team"], _e["bye"])
UPSIDE_KEYS = {normalize_name(n) for n in UPSIDE_POOL}
ALLOWED_TE_KEYS = {normalize_name(n) for n in ALLOWED_TES}
QB_FALL_KEYS = {normalize_name(n) for n in QB_VALUE_FALL_NAMES}
JEANTY_KEY = normalize_name("Ashton Jeanty")
WASHINGTON_KEY = normalize_name(JEANTY_BOOST_TARGET)

# ---------------------------------------------------------------------------
# Sleeper API
# ---------------------------------------------------------------------------
session = requests.Session()
session.headers["User-Agent"] = "draft-assistant/1.0"


def api_get(path, timeout=30, retries=3):
    """GET with retries; raises after exhausting retries (callers catch)."""
    last_err = None
    for attempt in range(retries):
        try:
            resp = session.get(API_BASE + path, timeout=timeout)
            resp.raise_for_status()
            return resp.json()
        except Exception as err:  # noqa: BLE001 - never crash mid-draft
            last_err = err
            time.sleep(min(2 ** attempt, 8))
    raise last_err


def load_players():
    """player_id -> {name, pos, team}. ~5MB download, cached for 24h."""
    if os.path.exists(PLAYERS_CACHE_FILE):
        age = time.time() - os.path.getmtime(PLAYERS_CACHE_FILE)
        if age < PLAYERS_CACHE_MAX_AGE:
            try:
                with open(PLAYERS_CACHE_FILE) as f:
                    return json.load(f)
            except (json.JSONDecodeError, OSError):
                pass  # corrupt cache; re-download
    print("Downloading Sleeper player map (~5MB, cached 24h)...")
    raw = api_get("/players/nfl", timeout=120)
    players = {}
    for pid, p in raw.items():
        if not isinstance(p, dict):
            continue
        pos = p.get("position") or ""
        name = p.get("full_name") or " ".join(
            x for x in (p.get("first_name"), p.get("last_name")) if x
        )
        if not name:
            continue
        players[pid] = {"name": name, "pos": pos, "team": p.get("team") or ""}
    tmp = PLAYERS_CACHE_FILE + ".tmp"
    with open(tmp, "w") as f:
        json.dump(players, f)
    os.replace(tmp, PLAYERS_CACHE_FILE)
    return players


# ---------------------------------------------------------------------------
# Draft state
# ---------------------------------------------------------------------------
def slot_for_pick(pick_no):
    """Snake order: 1..12, then 12..1, alternating."""
    rnd = (pick_no - 1) // TEAMS + 1
    idx = (pick_no - 1) % TEAMS
    return idx + 1 if rnd % 2 == 1 else TEAMS - idx


def next_my_pick(current_pick_no):
    for p in MY_PICKS:
        if p >= current_pick_no:
            return p
    return None


class DraftState:
    def __init__(self, players):
        self.players = players
        self.picks = []           # raw Sleeper pick dicts, in pick order
        self.drafted_keys = set() # normalized names of everyone drafted
        self.drafted_def_teams = set()
        self.my_roster = []       # my picks as info dicts

    def describe_pick(self, pick):
        pid = str(pick.get("player_id") or "")
        meta = pick.get("metadata") or {}
        info = self.players.get(pid)
        if info is None:
            name = " ".join(
                x for x in (meta.get("first_name"), meta.get("last_name")) if x
            ) or pid
            info = {"name": name, "pos": meta.get("position") or "?", "team": meta.get("team") or ""}
        board_entry = None
        if info["pos"] == "DEF":
            board_entry = BOARD_DEF_BY_TEAM.get(pid)
        else:
            cand = BOARD_BY_KEY.get(normalize_name(info["name"]))
            if cand and cand["pos"] == info["pos"]:
                board_entry = cand
        bye = board_entry["bye"] if board_entry else TEAM_BYES.get(info["team"])
        return {
            "player_id": pid,
            "name": board_entry["name"] if board_entry else info["name"],
            "pos": info["pos"],
            "team": board_entry["team"] if board_entry else info["team"],
            "bye": bye,
            "pick_no": pick.get("pick_no"),
            "round": pick.get("round"),
            "slot": pick.get("draft_slot"),
            "board_rank": board_entry["rank"] if board_entry else None,
        }

    def set_picks(self, picks):
        picks = sorted(picks, key=lambda p: p.get("pick_no") or 0)
        self.picks = picks
        self.drafted_keys = set()
        self.drafted_def_teams = set()
        self.my_roster = []
        for pick in picks:
            d = self.describe_pick(pick)
            if d["pos"] == "DEF":
                self.drafted_def_teams.add(d["player_id"])
            self.drafted_keys.add(normalize_name(d["name"]))
            is_mine = (pick.get("draft_slot") == MY_SLOT
                       or pick.get("picked_by") == USER_ID)
            if is_mine:
                self.my_roster.append(d)

    def current_pick_no(self):
        return len(self.picks) + 1

    def available_board(self):
        out = []
        for e in BOARD:
            if e["pos"] == "DEF":
                if e["team"] in self.drafted_def_teams:
                    continue
            elif e["key"] in self.drafted_keys:
                continue
            out.append(e)
        return out

    def roster_counts(self):
        counts = {"QB": 0, "RB": 0, "WR": 0, "TE": 0, "DEF": 0, "K": 0}
        for d in self.my_roster:
            counts[d["pos"]] = counts.get(d["pos"], 0) + 1
        return counts

    def week11_bye_count(self):
        return sum(1 for d in self.my_roster if d["bye"] == BYE_GUARD_WEEK)

    def i_have(self, key):
        return key in {normalize_name(d["name"]) for d in self.my_roster}

    def position_run(self, window=6, threshold=3):
        """Return list of (pos, count) with >=threshold in the last `window` picks."""
        recent = self.picks[-window:]
        counts = {}
        for pick in recent:
            pos = self.describe_pick(pick)["pos"]
            counts[pos] = counts.get(pos, 0) + 1
        return sorted(
            [(pos, n) for pos, n in counts.items() if n >= threshold],
            key=lambda x: -x[1],
        )


# ---------------------------------------------------------------------------
# Recommendation engine
# ---------------------------------------------------------------------------
def score_candidates(state, pick_no):
    """Rank every board player still available for MY pick at `pick_no`.

    Returns candidates sorted best-first: (adj_rank, entry, reason) tuples.
    Hard rules exclude players entirely; soft rules adjust the board rank.
    """
    counts = state.roster_counts()
    bye11 = state.week11_bye_count()
    have_jeanty = state.i_have(JEANTY_KEY)
    need_qb_now = counts["QB"] < 1 and pick_no >= QB_DEADLINE
    need_def_now = counts["DEF"] < 1 and pick_no >= DEF_DEADLINE

    results = []
    for e in state.available_board():
        pos, key = e["pos"], e["key"]
        reasons = []

        # --- hard exclusions ---
        if pos == "K":
            continue  # never a kicker (none on the board, but belt+suspenders)
        if pos == "TE":
            if key not in ALLOWED_TE_KEYS or counts["TE"] >= MAX_TE:
                continue
        if pos == "QB":
            if counts["QB"] >= MAX_QB:
                continue
            if pick_no < QB_ANY_PICK:
                if not (key in QB_FALL_KEYS and pick_no >= QB_VALUE_FALL_PICK):
                    continue
                reasons.append("elite QB value fall")
        if pos == "DEF":
            if counts["DEF"] >= MAX_DEF:
                continue
            if pick_no < DEF_WINDOW_START:
                continue
            reasons.append("DEF window %d-%d (SEA/DEN preferred)"
                           % (DEF_WINDOW_START, DEF_DEADLINE))

        adj = float(e["rank"])

        # --- deadline forcing: must-have positions jump the queue ---
        if need_qb_now and pos == "QB":
            adj -= 1000
            reasons.append("MUST take a QB by pick %d" % QB_DEADLINE)
        if need_def_now and pos == "DEF":
            adj -= 900
            reasons.append("MUST take a DEF by pick %d" % DEF_DEADLINE)

        # --- soft adjustments ---
        if pick_no <= BALANCE_THROUGH_PICK and pos in ("RB", "WR"):
            rb, wr = counts["RB"], counts["WR"]
            after = {"RB": (rb + 1, wr), "WR": (rb, wr + 1)}[pos]
            if abs(after[0] - after[1]) > 1:
                adj += BALANCE_PENALTY_SPOTS
                reasons.append("RB/WR balance penalty")
            elif abs(after[0] - after[1]) < abs(rb - wr):
                reasons.append("balances RB/WR")

        if bye11 >= BYE_GUARD_COUNT and e["bye"] == BYE_GUARD_WEEK:
            adj += BYE_PENALTY_SPOTS
            reasons.append("week-%d bye pileup penalty" % BYE_GUARD_WEEK)

        if have_jeanty and key == WASHINGTON_KEY:
            adj -= JEANTY_BOOST_SPOTS
            reasons.append("Jeanty handcuff/insurance boost")

        if pick_no >= UPSIDE_PICK_START and pos in ("RB", "WR") and key in UPSIDE_KEYS:
            adj -= UPSIDE_BOOST_SPOTS
            reasons.append("late-round upside pool")

        if e["note"] == "ankle":
            reasons.append("ankle - may miss early weeks (already discounted)")

        if not reasons:
            reasons.append("best value on board")
        results.append((adj, e, "; ".join(reasons)))

    results.sort(key=lambda r: (r[0], r[1]["rank"]))
    return results


def recommend(state, pick_no):
    """Return (pick1, pick2) as (entry, reason) tuples; either may be None."""
    ranked = score_candidates(state, pick_no)
    if not ranked:
        return None, None
    _, top, top_reason = ranked[0]
    pick1 = (top, top_reason)
    # Backup: next best, preferring a different position when possible.
    backup = None
    for adj, e, reason in ranked[1:]:
        if e["pos"] != top["pos"]:
            backup = (e, reason)
            break
    if backup is None and len(ranked) > 1:
        _, e, reason = ranked[1]
        backup = (e, reason)
    return pick1, backup


def watch_list(state, following_pick_no, exclude_keys, n=5):
    """Top candidates for my *following* pick, excluding current recs."""
    if following_pick_no is None:
        return []
    ranked = score_candidates(state, following_pick_no)
    out = []
    for _, e, reason in ranked:
        if e["key"] in exclude_keys:
            continue
        out.append((e, reason))
        if len(out) >= n:
            break
    return out


# ---------------------------------------------------------------------------
# Output: dashboard, notifications, logging
# ---------------------------------------------------------------------------
def fmt_player(e, with_rank=True):
    bits = ["%s %s" % (e["name"], e["pos"])]
    bits.append("(%s, bye %s%s)" % (e["team"], e["bye"],
                                    ", " + e["note"] if e["note"] else ""))
    if with_rank:
        bits.append("[board #%d]" % e["rank"])
    return " ".join(bits)


def notify_mac(message):
    """macOS notification + terminal bell. Silently no-ops elsewhere."""
    sys.stdout.write("\a")
    sys.stdout.flush()
    try:
        safe = message.replace('"', "'")
        subprocess.run(
            ["osascript", "-e",
             'display notification "%s" with title "Draft Assistant"' % safe],
            capture_output=True, timeout=5, check=False,
        )
    except (OSError, subprocess.SubprocessError):
        pass


def log_pick(desc):
    try:
        with open(LOG_FILE, "a") as f:
            f.write("%s  pick %s (R%s, slot %s): %s %s %s\n" % (
                datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
                desc["pick_no"], desc["round"], desc["slot"],
                desc["name"], desc["pos"],
                "(%s)" % desc["team"] if desc["team"] else "",
            ))
    except OSError:
        pass


def render_dashboard(state, draft_status, clear=True):
    lines = []
    now = datetime.now().strftime("%H:%M:%S")
    pick_no = state.current_pick_no()
    my_next = next_my_pick(pick_no)

    lines.append("=" * 74)
    lines.append(" SLEEPER DRAFT ASSISTANT - %s (slot %d) - Fantasy Friends - %s"
                 % (USERNAME, MY_SLOT, now))
    lines.append("=" * 74)

    if draft_status == "pre_draft":
        lines.append(" Waiting for draft to start... (polling every %ds)" % POLL_SECONDS)
    elif draft_status == "complete" or pick_no > TOTAL_PICKS:
        lines.append(" DRAFT COMPLETE.")
    else:
        on_clock_slot = slot_for_pick(pick_no)
        rnd = (pick_no - 1) // TEAMS + 1
        if my_next is None:
            lines.append(" Pick %d (R%d) - slot %d on the clock. My picks are done."
                         % (pick_no, rnd, on_clock_slot))
        else:
            until = my_next - pick_no
            lines.append(" Pick %d (R%d) - slot %d on the clock - %s"
                         % (pick_no, rnd, on_clock_slot,
                            ">>> MY PICK NOW! <<<" if until == 0
                            else "%d pick%s until mine (pick %d)"
                            % (until, "" if until == 1 else "s", my_next)))

    # Position run detector
    runs = state.position_run()
    if runs:
        lines.append(" !! POSITION RUN: " + ", ".join(
            "%d of last %d picks were %s" % (n, min(6, len(state.picks)), pos)
            for pos, n in runs))

    # Recommendations
    if my_next is not None and draft_status != "complete":
        header = " MY PICK NOW" if my_next == pick_no else " MY NEXT PICK (#%d)" % my_next
        lines.append("-" * 74)
        lines.append(header)
        pick1, pick2 = recommend(state, my_next)
        if pick1:
            lines.append("   1) %s" % fmt_player(pick1[0]))
            lines.append("      -> %s" % pick1[1])
        else:
            lines.append("   (no eligible players left on the board)")
        if pick2:
            lines.append("   2) %s  (backup)" % fmt_player(pick2[0]))
            lines.append("      -> %s" % pick2[1])

        # Watch list for the pick after that
        idx = MY_PICKS.index(my_next)
        following = MY_PICKS[idx + 1] if idx + 1 < len(MY_PICKS) else None
        if following:
            exclude = {pick1[0]["key"]} if pick1 else set()
            wl = watch_list(state, following, exclude)
            if wl:
                lines.append("-" * 74)
                lines.append(" WATCH LIST for my following pick (#%d):" % following)
                for i, (e, _reason) in enumerate(wl, 1):
                    lines.append("   %d. %s" % (i, fmt_player(e)))

    # My roster grouped by position
    lines.append("-" * 74)
    lines.append(" MY ROSTER (%d/%d)" % (len(state.my_roster), ROUNDS))
    if not state.my_roster:
        lines.append("   (empty)")
    else:
        by_pos = {}
        for d in state.my_roster:
            by_pos.setdefault(d["pos"], []).append(d)
        for pos in ("QB", "RB", "WR", "TE", "DEF"):
            group = by_pos.pop(pos, [])
            if group:
                lines.append("   %-3s: %s" % (pos, ", ".join(
                    "%s (bye %s)" % (d["name"], d["bye"] if d["bye"] else "?")
                    for d in group)))
        for pos, group in by_pos.items():  # anything unexpected
            lines.append("   %-3s: %s" % (pos, ", ".join(d["name"] for d in group)))
        bye11 = state.week11_bye_count()
        if bye11 >= BYE_GUARD_COUNT:
            lines.append("   ! %d players on the week-%d bye - penalizing more"
                         % (bye11, BYE_GUARD_WEEK))

    # Last few picks
    if state.picks:
        lines.append("-" * 74)
        lines.append(" LAST PICKS:")
        for pick in state.picks[-4:]:
            d = state.describe_pick(pick)
            mine = " <= ME" if d["slot"] == MY_SLOT else ""
            lines.append("   #%-3s slot %-2s %s %s%s"
                         % (d["pick_no"], d["slot"], d["name"], d["pos"], mine))
    lines.append("=" * 74)

    if clear:
        sys.stdout.write("\033[2J\033[H")
    print("\n".join(lines))
    sys.stdout.flush()


# ---------------------------------------------------------------------------
# Simulate mode
# ---------------------------------------------------------------------------
def fake_picks_for_simulation(n):
    """Pretend the first N board (ADP proxy) players are gone.

    Snake-assign them to slots so 'whose turn' and roster math stay coherent;
    any that would land on my slots become my roster (so recs reflect it).
    """
    picks = []
    for i, e in enumerate(BOARD[:n]):
        pick_no = i + 1
        pid = e["team"] if e["pos"] == "DEF" else "SIM%d" % e["rank"]
        picks.append({
            "player_id": pid,
            "pick_no": pick_no,
            "round": (pick_no - 1) // TEAMS + 1,
            "draft_slot": slot_for_pick(pick_no),
            "picked_by": USER_ID if slot_for_pick(pick_no) == MY_SLOT else "",
            "metadata": {"first_name": e["name"], "last_name": "",
                         "position": e["pos"], "team": e["team"]},
        })
    return picks


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
def main():
    ap = argparse.ArgumentParser(description="Sleeper live draft assistant")
    ap.add_argument("--simulate", type=int, metavar="N", default=None,
                    help="pretend the first N board/ADP picks are gone, "
                         "render the dashboard once, and exit")
    ap.add_argument("--once", action="store_true",
                    help="render one live dashboard and exit (connectivity test)")
    ap.add_argument("--poll", type=int, default=POLL_SECONDS,
                    help="poll interval in seconds (default %d)" % POLL_SECONDS)
    args = ap.parse_args()

    # Startup: player map (cached), draft sanity check.
    try:
        players = load_players()
        print("Player map loaded: %d players." % len(players))
        draft = api_get("/draft/%s" % DRAFT_ID)
        status = draft.get("status", "?")
        order = draft.get("draft_order") or {}
        my_slot_live = order.get(USER_ID)
        print("Draft '%s' status=%s; my slot per Sleeper: %s (expected %d)."
              % ((draft.get("metadata") or {}).get("name", "?"), status,
                 my_slot_live, MY_SLOT))
        if my_slot_live is not None and my_slot_live != MY_SLOT:
            print("WARNING: Sleeper says slot %s, config says %d - check MY_SLOT!"
                  % (my_slot_live, MY_SLOT))
        print("My overall picks: %s" % ", ".join(map(str, MY_PICKS)))
    except Exception as err:  # noqa: BLE001
        print("Startup fetch failed (%s). Will keep retrying in the loop." % err)
        players, status = {}, "?"

    state = DraftState(players)

    if args.simulate is not None:
        n = max(0, args.simulate)
        state.set_picks(fake_picks_for_simulation(n))
        print("\n--- SIMULATION: first %d board players drafted ---" % n)
        render_dashboard(state, "drafting", clear=False)
        return

    logged = set()
    notified_for_pick = None
    while True:
        try:
            draft = api_get("/draft/%s" % DRAFT_ID)
            status = draft.get("status", "?")
            picks = api_get("/draft/%s/picks" % DRAFT_ID) or []
        except Exception as err:  # noqa: BLE001
            print("[%s] API hiccup: %s - retrying in %ds"
                  % (datetime.now().strftime("%H:%M:%S"), err, args.poll))
            time.sleep(args.poll)
            continue

        state.set_picks(picks)

        # Log any newly seen picks.
        for pick in state.picks:
            no = pick.get("pick_no")
            if no not in logged:
                log_pick(state.describe_pick(pick))
                logged.add(no)

        render_dashboard(state, status)

        # Heads-up when my pick is close.
        pick_no = state.current_pick_no()
        my_next = next_my_pick(pick_no)
        if (status == "drafting" and my_next is not None
                and my_next - pick_no <= 3 and notified_for_pick != my_next):
            until = my_next - pick_no
            msg = ("YOU ARE ON THE CLOCK (pick %d)!" % my_next if until == 0
                   else "%d pick%s until yours (pick %d) - get ready!"
                   % (until, "" if until == 1 else "s", my_next))
            notify_mac(msg)
            notified_for_pick = my_next

        if args.once:
            return
        if status == "complete":
            print("Draft complete - good luck this season!")
            return
        time.sleep(args.poll)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\nBye - good luck in the draft!")
