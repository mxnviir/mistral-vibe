"""The /model picker lists a configured main+subagent mode as one row; picking
it asks the main model's thinking level, then the subagent model's own in a
second stage, and writes both levels with the two models in one pick."""

from __future__ import annotations

handshake = {
    "runtime/read": {"runtime": {"config": {
        "activeModel": {
            "name": "mistral-vibe-cli",
            "alias": "mistral-vibe-cli",
            "thinking": "off",
            "thinkingLevels": ["off", "low"],
        },
        "defaultModelAlias": "mistral-vibe-cli",
        "subagentModel": "glm-flash",
        "models": [
            {
                "alias": "mistral-vibe-cli",
                "thinking": "off",
                "thinkingLevels": ["off", "low"],
            },
            {
                "alias": "glm-flash",
                "thinking": "off",
                "thinkingLevels": ["off", "low"],
            },
        ],
        "modelProfiles": [
            {
                "name": "glm-mode",
                "main": "mistral-vibe-cli",
                "subagent": "glm-flash",
                "displayName": "GLM 5.3 main + GLM 5.3 Flash sub",
            },
        ],
    }}},
}

# The rows: Default, the two models, the profile, the Custom entry. Enter picks
# the profile, the next Enter answers the main model's stage, and the last one
# answers the subagent model's stage and commits the whole pick.
timeline: Timeline = ["/model", "\r", "\x1b[B", "\x1b[B", "\x1b[B", "\r", "\r", "\r"]

# The pick lands: the reloaded status names it done, and the goldens carry the
# profile row, the main model's stage, and the subagent's.
screen_contains = ("Configuration reloaded",)
