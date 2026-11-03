"""The /model picker lists a configured main+subagent mode as one row, and
picking it opens the thinking picker on its main model, holding both models."""

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

# The rows: Default, the two models, the profile, the Custom entry.
timeline: Timeline = ["/model", "\r", "\x1b[B", "\x1b[B", "\x1b[B", "\r"]

# The pick chains straight into the thinking picker on the profile's main
# model: its own two levels are what that model offers. The goldens carry the
# picker's rows, the profile row included.
screen_contains = (
    "Select Thinking Level",
    "Off",
    "Low",
)
