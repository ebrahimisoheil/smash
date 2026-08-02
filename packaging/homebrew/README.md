# Homebrew Tap Packaging

This directory contains the tap-ready Formula for Smash. Publish it in a
separate repository named `homebrew-Smash` so users can install Smash with:

```bash
brew tap ebrahimisoheil/smash
brew install Smash
```

The Formula installs Smash's CLI and local web runtime. It does not bundle the
MCP SDK; MCP clients should keep using the existing `smash-mcp` PyPI package or
the agent installers, which create the managed `~/.smash-mcp-venv`.

## Publish The Tap

Create the tap repository once:

```bash
brew tap-new ebrahimisoheil/smash
```

Copy the Formula into the tap:

```bash
cp packaging/homebrew/Formula/smash.rb "$(brew --repo ebrahimisoheil/smash)/Formula/smash.rb"
```

Validate locally:

```bash
brew audit --strict --online ebrahimisoheil/smash/Smash
brew install --build-from-source ebrahimisoheil/smash/Smash
brew test ebrahimisoheil/smash/Smash
smash --version
smash demo
```

Then push the tap repo:

```bash
cd "$(brew --repo ebrahimisoheil/smash)"
git status --short
git add Formula/smash.rb
git commit -m "Add Smash formula"
git push origin main
```

## Update For A New Release

1. Tag the Smash repo release.
2. Update `tag` and `revision` in `Formula/smash.rb`.
3. Copy the Formula into the tap repo.
4. Run `brew audit`, `brew install --build-from-source`, and `brew test`.
5. Push the tap repo.
