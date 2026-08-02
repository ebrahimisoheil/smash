# SmashBar release checklist (rides the Smash 2.0.0 release)

1. `python3 scripts/prepare_release.py 2.0.0` on develop -> commit -> push
2. PR develop -> main, wait for all 10 CI checks, merge (merge commit)
3. `git switch main && git pull --ff-only && git tag -a v2.0.0 -m v2.0.0 && git push origin v2.0.0`
4. PyPI + mcp-publisher (same as 1.7 runbook)
5. Formula bump in the tap (url/sha for v2.0.0 tarball) AND replace the
   caveats block with the version below — it is the only place a
   `brew install` user learns SmashBar exists, and the MCP paragraph is
   obsolete (1.7 self-provisions ~/.smash-mcp-venv):

   ```ruby
   def caveats
     <<~EOS
       Try Smash:
         smash proof                 # prove cross-agent memory in ~1 second
         smash try                   # the full demo wiki

       Wire your agent (creates ~/Smash, provisions MCP, writes hooks):
         smash onboard --agent claude-code --hooks --write

       Optional, macOS: put the review gate in your menu bar —
       notifications when memory is captured, a global palette (Opt-Cmd-M),
       and a live view of every Smash surface:
         brew install --cask ebrahimisoheil/smash/smashbar

       Optional: meaning-based recall (one-time local model download):
         smash semantic ~/Smash --setup
     EOS
   end
   ```

6. **SmashBar zip**: `cd apps/SmashBar && bash Scripts/bundle.sh --release-zip`
   - attach `.build/SmashBar-1.0.0.zip` to the v2.0.0 GitHub release
7. **Cask**: copy `packaging/smashbar.rb` to the tap as `Casks/smashbar.rb`,
   paste the sha256 from step 6, `git push` the tap
8. Verify as user #1: `brew install --cask ebrahimisoheil/smash/smashbar`
   -> app opens with no Gatekeeper dialog; menu icon appears
9. Back-merge main -> develop
10. QA before step 1, on the installed app:
    - press ⌥⌘M anywhere -> palette appears, recall works, `+text` remembers
    - end an agent session with a memorable line -> notification banner with
      Accept appears; Accept works from the banner
    - Status tab all green
