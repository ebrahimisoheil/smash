#!/usr/bin/env python3
"""Smash local HTTP API runner. python api.py -> http://127.0.0.1:3001/api"""
from __future__ import annotations

import sys

import serve


DEFAULT_API_PORT = 3001


def main() -> None:
    port, root = serve._parse_serve_args(
        sys.argv[1:],
        default_port=DEFAULT_API_PORT,
        default_root=serve.ROOT,
    )
    raise SystemExit(
        serve.run_local_server(
            port=port,
            root=root,
            handler_class=serve.APIHandler,
            startup_lines=serve._api_startup_lines,
        )
    )


if __name__ == "__main__":
    main()
