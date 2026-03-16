Fix the heartbeat and put these back:

/Users/ericg/.claude/settings.json
```
      "PreToolUse": [
        {
          "matcher": "blue_*",
          "hooks": [
            {
              "type": "command",
              "command": "blue session-heartbeat"
            }
          ]
        }
      ],
      "SessionEnd": [
        {
          "matcher": "",
          "hooks": [
            {
              "type": "command",
              "command": "blue session-end"
            }
          ]
        }
      ]
```
```
{
  "hooks": {
    "PreToolUse": [
      {
        "hooks": [
          {
            "command": ".claude/hooks/guard-write.sh",
            "type": "command"
          }
        ],
        "matcher": "Write|Edit|MultiEdit"
      }
    ],
    "SessionStart": [
      {
        "hooks": [
          {
            "command": ".claude/hooks/session-start.sh",
            "type": "command"
          }
        ]
      }
    ]
  }
}
```


/Users/ericg/letemcook/blue/.claude/settings.json
```
      {
        "matcher": "Write|Edit|MultiEdit",
        "hooks": [
          {
            "type": "command",
            "command": "blue guard --path=\"$TOOL_INPUT:file_path\""
          }
        ]
      }
```