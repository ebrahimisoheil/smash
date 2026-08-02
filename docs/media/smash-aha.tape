# Smash "aha" demo — the paraphrase-recall moment, for the GitHub README.
#
# Renders a crisp, deterministic GIF (no synthetic frames) with charmbracelet vhs.
#
# One-time render (needs ffmpeg + ttyd, pulled in by `brew install vhs`):
#   brew install vhs
#   pip install model2vec        # the fast local semantic tier
#   cd docs/media && vhs smash-aha.tape
# Then wire it into README.md:
#   <img src="docs/assets/smash-aha.gif" alt="Smash recall finds a memory phrased in different words" width="720">
# and add "smash-aha.gif" to REQUIRED_ASSETS in scripts/generate_docs_media.py.

Output ../assets/smash-aha.gif

Require smash

Set Shell bash
Set FontSize 20
Set Width 1280
Set Height 500
Set Padding 44
Set Theme { "background": "#221c12", "foreground": "#f3ece0", "cursor": "#e0955f", "black": "#221c12", "green": "#86c79a", "brightBlack": "#8a8174", "white": "#f3ece0", "blue": "#e0955f", "brightBlue": "#e0955f", "cyan": "#d9b48c", "brightCyan": "#d9b48c" }

# ── prep the demo workspace off-screen; make it the working directory ──
Hide
Type "O=$PWD; L=$O/.aha-demo; rm -rf $L; mkdir -p $L; cd $L" Enter
Type "smash init . >/dev/null 2>&1" Enter
Type "smash remember 'Name branches feat/short-topic, never long' . --type preference >/dev/null 2>&1" Enter
Type "smash semantic . --setup >/dev/null 2>&1" Enter
Type "clear" Enter
Show

# ── the moment: ask in totally different words, it finds it ──
Sleep 900ms
Type@85ms "smash recall 'how should I name my git branches'"
Sleep 600ms
Enter
Sleep 3s
Type@85ms "# different words, same memory - matched by meaning, not keywords"
Sleep 2400ms

# ── cleanup off-screen ──
Hide
Type "cd $O; rm -rf $L" Enter
