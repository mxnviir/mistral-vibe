"""The /agent picker lists the main agents and the subagent role in separate
sections; picking a subagent role writes the session's default subagent, and
picking a main agent switches the session live."""

from __future__ import annotations

import json
from copy import deepcopy

from e2e.app_server.config import FIXTURE_PATH
from e2e.app_server.scenario import Timeline

# A subagent pick writes /default_subagent and reloads; the reload sees it.
runtime = json.loads(FIXTURE_PATH.read_text())["handshake"]["runtime/read"]["runtime"]
written = deepcopy(runtime)
written["config"]["defaultSubagent"] = "explore"

handshake = {
    "config/write": {"runtime": written},
    "config/reload": {"runtime": {"config": {"defaultSubagent": "explore"}}},
}

_DOWN = "\x1b[B"

# Rows: MAIN AGENT caption, Default, the four primaries, SUBAGENT ROLE caption,
# Inherit, Explore, Suggested. Open on the Default row; six downs walk the
# primaries and skip the caption to Explore, and Enter makes it the session's
# default. Reopening starts on the Default row again (the main agent's state);
# three downs reach Smart Approve, and Enter switches the session live.
timeline: Timeline = [
    "/agent",
    "\r",
    _DOWN,
    _DOWN,
    _DOWN,
    _DOWN,
    _DOWN,
    _DOWN,
    "\r",
    "/agent",
    "\r",
    _DOWN,
    _DOWN,
    _DOWN,
    "\r",
]

# The reload keeps its own quiet path: the write and the switch are pinned in
# requests.json; the switch's landing shows in the prompt marker.
screen_contains = ("smart approve",)
request_methods = {"config/write", "config/reload", "session/agent/update"}
