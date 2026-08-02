import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SKILLS_DIR = ROOT / "skills"

EXPECTED_SKILLS = {
    "smash-health": ("smash health", "smash operations", "smash backup", "smash validate"),
    "smash-retrieve": ("smash query", "smash brief", "smash graph-summary", "smash benchmark"),
    "smash-ingest": ("smash ingest-status", "smash propose-memories", "smash rebuild-index", "smash validate"),
    "smash-memory": ("smash brief", "smash recall", "smash session-end", "smash remember", "smash memory-inbox"),
}


def read_skill(name: str) -> str:
    return (SKILLS_DIR / name / "SKILL.md").read_text(encoding="utf-8")


def parse_frontmatter(text: str) -> dict[str, str]:
    if not text.startswith("---\n"):
        return {}
    _, frontmatter, _body = text.split("---", 2)
    parsed: dict[str, str] = {}
    for line in frontmatter.splitlines():
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        parsed[key.strip()] = value.strip()
    return parsed


class OfficialSkillsTests(unittest.TestCase):
    def test_expected_skills_exist_with_valid_frontmatter(self):
        for name in EXPECTED_SKILLS:
            with self.subTest(skill=name):
                path = SKILLS_DIR / name / "SKILL.md"
                self.assertTrue(path.exists(), f"missing official skill: {path}")

                frontmatter = parse_frontmatter(read_skill(name))
                self.assertEqual(frontmatter.get("name"), name)
                self.assertGreaterEqual(len(frontmatter.get("description", "")), 60)

    def test_skills_are_cli_backed_and_lazy_loadable(self):
        forbidden_claims = (
            "mcp is required",
            "requires mcp",
            "start the mcp server",
            "must run `smash serve`",
            "must run smash serve",
        )
        for name, commands in EXPECTED_SKILLS.items():
            with self.subTest(skill=name):
                text = read_skill(name)
                lower = text.lower()
                self.assertIn("smash", text)
                self.assertIn("[Smash-root]", text)
                self.assertIn("MCP", text)
                for command in commands:
                    self.assertIn(command, text)
                for forbidden in forbidden_claims:
                    self.assertNotIn(forbidden, lower)

    def test_skills_have_ambient_agent_triggers(self):
        expectations = {
            "smash-health": ("start", "readiness", "installs"),
            "smash-retrieve": ("before answering", "first substantive turn", "prior Smash memory"),
            "smash-ingest": ("raw files", "drops files", "learn next"),
            "smash-memory": ("important user-approved decisions", "propose first", "durable memory"),
        }
        passive_only_phrases = (
            "use when a user asks",
            "use when a user wants",
            "use when users ask",
        )
        for name, required in expectations.items():
            with self.subTest(skill=name):
                text = read_skill(name)
                lower = text.lower()
                for phrase in passive_only_phrases:
                    self.assertNotIn(phrase, lower)
                for phrase in required:
                    self.assertIn(phrase.lower(), lower)

    def test_docs_and_readme_reference_official_skills(self):
        readme = (ROOT / "README.md").read_text(encoding="utf-8")
        docs = (ROOT / "docs/skills.html").read_text(encoding="utf-8")

        self.assertIn("https://ebrahimisoheil.github.io/Smash/skills.html", readme)
        self.assertIn("Lazy-load Smash through skills", docs)
        for name in EXPECTED_SKILLS:
            with self.subTest(skill=name):
                skill_path = f"skills/{name}/SKILL.md"
                self.assertIn(skill_path, readme)
                self.assertIn(skill_path, docs)
