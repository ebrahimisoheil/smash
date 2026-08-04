---
name: engrave-connectors
version: 1.0.0
description: Explain the read-heavy connector lifecycle without handling secrets.
---

# ENGRAVE connectors

Connector credentials are configured through the host or secret store, never
in prompts or tool arguments. The initial connector is read-heavy and
worker-synchronized: stable external IDs, permission narrowing, cursors,
retries, and deletion are recorded as governed Source versions.
