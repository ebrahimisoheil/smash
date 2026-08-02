"""Shared first-run prompt helpers for Smash."""
from __future__ import annotations

from pathlib import Path

from .memory import default_project_for_target, normalize_project
from .mcp_verify import display_command


def _command_target(target: Path) -> Path:
    if target.name == "wiki" and (target / "index.md").exists():
        return target.parent
    return target


def starter_prompt_payload(target: Path, project: str | None = None) -> dict[str, object]:
    """Return natural agent prompts and local checks for a Smash user."""
    target = target.expanduser().resolve()
    command_target = str(_command_target(target))
    project_name = normalize_project(project) if project is not None else default_project_for_target(target)
    remember_prompt = (
        "remember that this project uses Smash for local agent memory"
        if project_name
        else "remember that I prefer local-first agent memory"
    )
    query_prompt = (
        "what does Smash remember about this project?"
        if project_name
        else "what does Smash know about me?"
    )
    prompts = [
        {
            "label": "Check readiness",
            "prompt": "is Smash ready?",
            "when": "right after install or before troubleshooting",
        },
        {
            "label": "Start with Smash",
            "prompt": "start with Smash before we continue",
            "when": "at the start of a session or task",
        },
        {
            "label": "Seed project context",
            "prompt": "seed this project into Smash",
            "when": "after install inside a repo, before the first real project recall",
        },
        {
            "label": "Save explicit memory",
            "prompt": remember_prompt,
            "when": "when you want future agents to remember a preference, decision, or project fact",
        },
        {
            "label": "Ask with context",
            "prompt": query_prompt,
            "when": "when you want a compact answer-ready packet from memory and wiki context",
        },
        {
            "label": "Ingest a source",
            "prompt": "ingest raw/<file> into Smash",
            "when": "after dropping a source file into raw/",
        },
        {
            "label": "Review memory proposals",
            "prompt": "propose memories from raw/<file>",
            "when": "when a source may contain preferences, decisions, or project context",
        },
    ]
    return {
        "target": str(target),
        "project": project_name,
        "shortcut": display_command(["Smash", "next", command_target]),
        "prompts": prompts,
        "commands": [
            display_command(["Smash", "seed", ".", command_target]),
            display_command(["Smash", "health", command_target]),
            display_command(["Smash", "ingest-status", command_target]),
            display_command(["Smash", "memory-inbox", command_target]),
            display_command(["Smash", "benchmark", "agent memory", command_target]),
        ],
    }


def welcome_payload(target: Path, project: str | None = None) -> dict[str, object]:
    """Return a short first-use path for a human trying Smash with an agent."""
    starter = starter_prompt_payload(target, project=project)
    command_target = str(_command_target(target.expanduser().resolve()))
    prompts = [
        item for item in starter.get("prompts", [])
        if isinstance(item, dict)
    ]
    proof = [
        "Agent can find Smash and check readiness.",
        "Agent can prime itself with compact local memory.",
        "Agent can save explicit memory only when you ask.",
    ]
    steps = []
    for index, item in enumerate(prompts[:3], start=1):
        steps.append({
            "step": index,
            "label": item.get("label", ""),
            "prompt": item.get("prompt", ""),
            "proves": proof[index - 1],
        })
    return {
        "target": starter["target"],
        "project": starter["project"],
        "steps": steps,
        "commands": [
            display_command(["Smash", "health", command_target]),
            display_command(["Smash", "serve", command_target]),
            display_command(["Smash", "ingest-status", command_target]),
            display_command(["Smash", "prompts", command_target]),
        ],
        "urls": [
            "http://127.0.0.1:3000",
            "http://127.0.0.1:3000/onboard",
            "http://127.0.0.1:3000/health",
            "http://127.0.0.1:3000/graph",
        ],
    }
