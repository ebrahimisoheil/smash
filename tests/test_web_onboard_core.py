import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "mcp_package"))

from smash_core.web_onboard import render_onboard_page  # noqa: E402


def _layout(title: str, body: str) -> str:
    return f"<title>{title}</title>{body}"


def test_render_onboard_page_shows_first_run_loop(tmp_path):
    wiki = tmp_path / "wiki"
    html = render_onboard_page(
        {
            "wiki": str(wiki),
            "ready": True,
            "content_page_count": 2,
            "memory_count": 1,
            "needs_review_count": 1,
            "validation": {"checked": True, "passed": True},
        },
        {"wiki": str(wiki), "operations": []},
        {
            "prompts": [
                {"label": "Check readiness", "prompt": "is Smash ready?", "when": "before work"},
                {"label": "Start with Smash", "prompt": "start with Smash before we continue", "when": "session start"},
            ]
        },
        target=str(tmp_path),
        agents=("codex", "cursor"),
        layout=_layout,
    )

    assert "<title>Onboard</title>" in html
    assert "project context" in html
    assert '<span class="label">ready</span>' in html
    assert "Check readiness" in html
    assert "Seed this project" in html
    assert "--seed-project" in html
    assert "smash seed ." in html
    assert "Run Setup Here" in html
    assert 'data-seed-project-form' in html
    assert 'data-first-memory-form' in html
    assert "Seed one memory" in html
    assert "Connect an agent" in html
    assert "MCP and CLI work without the viewer running" in html
    assert "smash health" in html
    assert "smash onboard" in html
    assert "--first-memory" in html
    assert "--agent codex" in html
    assert "--agent cursor" in html
    assert "--write" in html
    assert "is Smash ready?" in html
    assert "start with Smash before we continue" in html


def test_render_onboard_page_escapes_prompt_text(tmp_path):
    wiki = tmp_path / "wiki"
    html = render_onboard_page(
        {"wiki": str(wiki), "ready": False, "validation": {"checked": True, "passed": False}},
        {"wiki": str(wiki), "operations": []},
        {"prompts": [{"label": "<bad>", "prompt": "<script>alert(1)</script>", "when": "<now>"}]},
        target=str(tmp_path),
        agents=("codex",),
        layout=_layout,
    )

    assert "<script>alert(1)</script>" not in html
    assert "&lt;script&gt;alert(1)&lt;/script&gt;" in html
    assert "&lt;bad&gt;" in html
