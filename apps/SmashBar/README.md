# SmashBar

The review gate in your menu bar. Smash's promise is that nothing becomes
durable memory without your approval; SmashBar makes approving ambient
instead of a chore.

- Badge: pending-review count · Popover: the review inbox with one-click
  approve (mark reviewed) and archive
- Quick recall ("What do I know about…") with honest abstention — when the
  memory has nothing reliable, it says so
- Backend: the `smash` CLI's `--json` output. No server, no sockets, no new
  API surface. Workspace: `SMASH_WORKSPACE` or `~/Smash`.

## Build & run

```
cd apps/SmashBar
swift build                 # debug binary
./Scripts/bundle.sh         # release .app bundle at .build/SmashBar.app
open .build/SmashBar.app
```

Requires macOS 14+, Swift 5.10+, and Smash installed (`brew install
ebrahimisoheil/smash/Smash`). Status: early preview on the feature/menubar
branch — not yet part of a release.
