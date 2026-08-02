import unittest

from mcp_package.smash_core.cli_runtime import (
    render_demo_text,
    render_init_text,
    render_mcp_connect_text,
    render_onboard_text,
    render_proof_text,
    render_start_text,
    render_starter_prompts_text,
    render_try_text,
    render_welcome_text,
)


class CliRuntimeCoreTests(unittest.TestCase):
    def test_render_init_text(self):
        code, text = render_init_text(target="/tmp/Smash", fixes=["created wiki/index.md"])

        self.assertEqual(code, 0)
        self.assertIn("Smash wiki ready at /tmp/Smash", text)
        self.assertIn("Initialized:", text)
        self.assertIn("smash health /tmp/Smash", text)
        self.assertIn("smash onboard /tmp/Smash", text)
        self.assertIn("smash serve /tmp/Smash", text)
        self.assertIn("http://127.0.0.1:3000/onboard", text)

    def test_render_starter_prompts_text(self):
        code, text = render_starter_prompts_text({
            "target": "/tmp/Smash",
            "project": "Smash",
            "shortcut": "smash next /tmp/Smash",
            "prompts": [{
                "prompt": "is Smash ready?",
                "when": "first run",
            }],
            "commands": ["smash health"],
        })

        self.assertEqual(code, 0)
        self.assertIn("Smash starter prompts: /tmp/Smash", text)
        self.assertIn("Project: Smash", text)
        self.assertIn("Shortcut", text)
        self.assertIn("- smash next /tmp/Smash", text)
        self.assertIn("- is Smash ready?", text)
        self.assertIn("- smash health", text)

    def test_render_welcome_text(self):
        code, text = render_welcome_text({
            "target": "/tmp/Smash",
            "project": "Smash",
            "steps": [{
                "step": 1,
                "prompt": "is Smash ready?",
                "proves": "Agent can find Smash.",
            }],
            "commands": ["smash health"],
            "urls": ["http://127.0.0.1:3000/health"],
        })

        self.assertEqual(code, 0)
        self.assertIn("Smash welcome: /tmp/Smash", text)
        self.assertIn("Project: Smash", text)
        self.assertIn("1. is Smash ready?", text)
        self.assertIn("Proves: Agent can find Smash.", text)
        self.assertIn("- smash health", text)
        self.assertIn("- http://127.0.0.1:3000/health", text)

    def test_render_start_text(self):
        code, text = render_start_text({
            "target": "/tmp/Smash",
            "task": "release work",
            "status": {
                "ready": True,
                "content_page_count": 12,
                "page_count": 14,
                "active_memory_count": 2,
                "needs_review_count": 1,
                "search_backend": "sqlite-fts",
                "validation": {"checked": True, "passed": True},
            },
            "brief_text": "Smash memory brief: release work\n- Prefer short release notes",
            "commands": {
                "query": "smash query 'release work' /tmp/Smash --budget micro",
                "review": "smash memory-inbox /tmp/Smash",
            },
        })

        self.assertEqual(code, 0)
        self.assertIn("Smash start: /tmp/Smash", text)
        self.assertIn("Ready: yes", text)
        self.assertIn("Pages: 12 content", text)
        self.assertIn("Smash memory brief: release work", text)
        self.assertIn("smash query", text)

    def test_render_start_text_recommends_project_seed_when_context_is_empty(self):
        code, text = render_start_text({
            "target": "/tmp/Smash",
            "task": "new repo work",
            "status": {
                "ready": True,
                "content_page_count": 0,
                "page_count": 2,
                "active_memory_count": 0,
                "needs_review_count": 0,
                "search_backend": "sqlite-fts",
                "validation": {"checked": True, "passed": True},
            },
            "brief_text": "Smash memory brief: new repo work\nNo directly relevant memory found.",
            "commands": {
                "query": "smash query 'new repo work' /tmp/Smash --budget micro",
                "review": "smash memory-inbox /tmp/Smash",
            },
            "project_seed": {
                "recommended": True,
                "command": "smash seed . /tmp/Smash",
                "reason": "No source-backed project context or relevant memory found.",
                "safety": "Run from the project repo.",
            },
        })

        self.assertEqual(code, 0)
        self.assertIn("Seed project context: smash seed . /tmp/Smash", text)
        self.assertIn("No source-backed project context", text)
        self.assertIn("Run from the project repo.", text)
        self.assertLess(text.index("Seed project context"), text.index("Need more context"))

    def test_render_start_text_includes_tiny_context_preview(self):
        code, text = render_start_text({
            "target": "/tmp/Smash",
            "task": "release work",
            "status": {
                "ready": True,
                "content_page_count": 3,
                "page_count": 5,
                "active_memory_count": 0,
                "needs_review_count": 0,
                "search_backend": "sqlite-fts",
                "validation": {"checked": True, "passed": True},
            },
            "brief_text": "Smash memory brief: release work\n- none",
            "context_preview": {
                "budget": "micro",
                "recall_capsule": {
                    "estimated_tokens": 96,
                    "items": [{
                        "kind": "page",
                        "title": "Project seed: Smash",
                        "summary": "README context says Smash gives agents local memory.",
                    }],
                },
            },
            "commands": {
                "query": "smash query 'release work' /tmp/Smash --budget micro",
                "review": "smash memory-inbox /tmp/Smash",
            },
        })

        self.assertEqual(code, 0)
        self.assertIn("Context preview (micro · ~96 tokens)", text)
        self.assertIn("Project seed: Smash (page)", text)
        self.assertIn("README context says Smash gives agents local memory.", text)

    def test_render_demo_text(self):
        code, text = render_demo_text(
            target="/tmp/smash-demo",
            guide_path="/tmp/smash-demo/START_HERE.md",
            serve_command="python3 smash.py serve /tmp/smash-demo",
            next_command="python3 smash.py next /tmp/smash-demo",
            start_command="python3 smash.py start /tmp/smash-demo --task 'working on agent memory'",
            query_command="python3 smash.py query 'why does Smash help agents?' /tmp/smash-demo --budget small",
            brief_command="python3 smash.py brief 'working on agent memory' /tmp/smash-demo",
            audit_command="python3 smash.py memory-audit /tmp/smash-demo",
        )

        self.assertEqual(code, 0)
        self.assertIn("Smash demo created at /tmp/smash-demo", text)
        self.assertIn("Ask an agent what to try next:", text)
        self.assertIn("python3 smash.py next /tmp/smash-demo", text)
        self.assertIn("Try the value loop:", text)
        self.assertIn("python3 smash.py start /tmp/smash-demo", text)
        self.assertIn("/tmp/smash-demo/START_HERE.md", text)
        self.assertIn("http://127.0.0.1:3000/onboard", text)
        self.assertIn("http://127.0.0.1:3000/graph", text)

    def test_render_try_text(self):
        code, text = render_try_text(
            target="/tmp/smash-demo",
            ready=True,
            page_count=13,
            memory_count=1,
            search_backend="sqlite-fts",
            query_summary="agent-memory · 1 memory · 3 context items",
            brief_summary="1 relevant memory · 1 review item",
            serve_command="smash serve /tmp/smash-demo",
            next_command="smash next /tmp/smash-demo",
            health_command="smash health /tmp/smash-demo",
            query_command="smash query 'why does Smash help agents?' /tmp/smash-demo --budget small",
            brief_command="smash brief 'working on agent memory' /tmp/smash-demo",
            benchmark_command="smash benchmark 'agent memory' /tmp/smash-demo",
            url="http://127.0.0.1:3000",
        )

        self.assertEqual(code, 0)
        self.assertIn("Smash try: /tmp/smash-demo", text)
        self.assertIn("60-second proof complete", text)
        self.assertIn("Status", text)
        self.assertIn("Demo: ready", text)
        self.assertIn("13 pages · 1 memory", text)
        self.assertIn("Privacy: no cloud account", text)
        self.assertIn("What Smash proved", text)
        self.assertIn("Query proof:", text)
        self.assertIn("Agent path: CLI works now", text)
        self.assertIn("Ask an agent:", text)
        self.assertIn("http://127.0.0.1:3000/onboard", text)
        self.assertIn("smash next /tmp/smash-demo", text)

    def test_render_proof_text(self):
        code, text = render_proof_text({
            "target": "/tmp/Smash-proof",
            "created": True,
            "ready": True,
            "memory": {
                "created": True,
                "reviewed": True,
                "title": "Cross-agent Smash proof",
            },
            "recall": {"found": True},
            "prompts": {
                "agent_a": "remember that I want Smash memory shared across my local agents",
                "agent_b": "start with Smash before we continue",
            },
            "commands": {
                "start": "smash start /tmp/Smash-proof --task 'cross-agent proof'",
                "recall": "smash query 'cross-agent proof local memory' /tmp/Smash-proof --budget micro",
                "mcp": "smash connect codex /tmp/Smash-proof",
                "serve": "smash serve /tmp/Smash-proof --port 3000",
            },
        })

        self.assertEqual(code, 0)
        self.assertIn("Cross-agent memory continuity works", text)
        self.assertIn("throwaway demo wiki", text)
        self.assertIn("What this means for you", text)
        self.assertIn("Memory: created and reviewed", text)
        self.assertIn("same bounded recall path used by CLI, skills, and MCP", text)
        self.assertIn("Try it with two agents", text)
        self.assertIn("No viewer required", text)
        self.assertIn("Result: proof passed", text)

    def test_render_onboard_text_preview(self):
        code, text = render_onboard_text({
            "target": "/tmp/Smash",
            "created": True,
            "fixes": ["created wiki/index.md"],
            "status": {
                "ready": True,
                "content_page_count": 0,
                "memory_count": 1,
            },
            "first_memory": {
                "created": True,
                "path": "wiki/memories/prefer-local-memory.md",
            },
            "connections": [{
                "display_name": "Codex",
                "config_path": "/tmp/config.toml",
                "restart_hint": "Restart Codex, then ask: is Smash ready?",
                "write": {"requested": False, "ok": False},
                "next_actions": [{
                    "label": "write config",
                    "command_text": "smash connect codex /tmp/Smash --write",
                }],
            }],
            "prompts": [
                {"prompt": "is Smash ready?"},
                {"prompt": "start with Smash before we continue"},
            ],
            "commands": {
                "health": "smash health /tmp/Smash",
                "serve": "smash serve /tmp/Smash --port 3000",
                "memory_inbox": "smash memory-inbox /tmp/Smash",
                "ingest_status": "smash ingest-status /tmp/Smash",
            },
            "agent_examples": [],
            "url": "http://127.0.0.1:3000",
        })

        self.assertEqual(code, 0)
        self.assertIn("Smash onboard: /tmp/Smash", text)
        self.assertIn("Workspace", text)
        self.assertIn("saved for review", text)
        self.assertIn("Codex: preview", text)
        self.assertIn("Write when ready: smash connect codex /tmp/Smash --write", text)
        self.assertIn("After writing: Restart Codex", text)
        self.assertIn("is Smash ready?", text)
        self.assertIn("smash serve /tmp/Smash --port 3000", text)

    def test_render_onboard_write_without_agent_is_actionable_error(self):
        code, text = render_onboard_text({
            "target": "/tmp/Smash",
            "created": False,
            "status": {
                "ready": True,
                "content_page_count": 0,
                "memory_count": 0,
            },
            "write_requested": True,
            "connections": [],
            "prompts": [],
            "commands": {},
            "agent_examples": ["smash onboard /tmp/Smash --agent codex"],
        })

        self.assertEqual(code, 1)
        self.assertIn("no agent selected", text)
        self.assertIn("--agent codex", text)

    def test_render_mcp_connect_text_preview(self):
        code, text = render_mcp_connect_text({
            "display_name": "Codex",
            "wiki": "/tmp/smash/wiki",
            "python": "/tmp/python",
            "config_path": "/tmp/config.toml",
            "snippet": "[mcp_servers.Smash]\ncommand = \"/tmp/python\"",
            "write": {"requested": False, "ok": False, "message": "preview only"},
            "next_actions": [
                {"label": "write config", "command_text": "smash connect codex /tmp/Smash --write"},
                {"label": "verify MCP runtime", "command_text": "smash verify-mcp /tmp/Smash --python /tmp/python"},
            ],
            "restart_hint": "Restart the agent, then ask: is Smash ready?",
        })

        self.assertEqual(code, 0)
        self.assertIn("Smash connect: Codex", text)
        self.assertIn("Preview only", text)
        self.assertIn("smash connect codex /tmp/Smash --write", text)
        self.assertIn("[mcp_servers.Smash]", text)
        self.assertIn("smash verify-mcp /tmp/Smash --python /tmp/python", text)


if __name__ == "__main__":
    unittest.main()
