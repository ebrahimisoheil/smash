# ADR-0010: Configuration layering and secret handling

Status: Accepted  
Date: 2026-08-02

## Decision

Configuration resolves in this order: compiled defaults → optional file →
environment variables → secret references. The final configuration is parsed
and validated before a service binds a listener or accepts work. Missing
required values and invalid combinations fail boot with a non-sensitive error.

Secrets are referenced, not stored in source control or database plaintext.
Connector credentials are encrypted with a rotatable application key; logs and
errors never print secret values.

## Consequences

Each binary has one validated configuration object. Tests may inject a fully
constructed value without reading the environment. Secret rotation changes the
reference/material without changing domain identifiers.
