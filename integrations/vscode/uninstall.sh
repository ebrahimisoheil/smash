#!/bin/bash
# Remove Smash instructions from VS Code settings (preserves other instructions)
set -e

TARGET=".vscode/settings.json"
if [ ! -f "$TARGET" ]; then echo "No $TARGET found"; exit 0; fi

python3 -c "
import json
settings = json.load(open('$TARGET'))
instructions = settings.get('github.copilot.chat.codeGeneration.instructions', [])
filtered = [
    i for i in instructions
    if '## Smash — Local Agent Memory' not in i.get('text', '')
    and '## Smash — Personal Knowledge Wiki' not in i.get('text', '')
    and 'Smash, an LLM-maintained knowledge wiki' not in i.get('text', '')
]
if len(filtered) < len(instructions):
    if filtered:
        settings['github.copilot.chat.codeGeneration.instructions'] = filtered
    else:
        del settings['github.copilot.chat.codeGeneration.instructions']
    json.dump(settings, open('$TARGET', 'w'), indent=2)
    print('Smash instructions removed from $TARGET')
else:
    print('No Smash instructions found in $TARGET')
"
