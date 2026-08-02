param([switch]$Project)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $PSCommandPath
. (Join-Path $ScriptDir "..\_shared\instructions.ps1")

$mode = if ($Project) { "--project" } else { "--global" }
$instructionsFile = if ($Project) {
    Join-Path $ScriptDir "..\_shared\smash-instructions-project.md"
} else {
    Join-Path $ScriptDir "..\_shared\smash-instructions.md"
}
$target = if ($Project) { "AGENTS.md" } else { Join-Path $HOME "AGENTS.md" }
$wikiPath = if ($Project) { Join-Path (Get-Location).Path "wiki" } else { Join-Path $HOME "Smash\wiki" }

Smash-UpsertInstructions $target $instructionsFile "Smash instructions"

if ($Project) {
    & (Join-Path $ScriptDir "..\_shared\scaffold.ps1") -Project
} else {
    & (Join-Path $ScriptDir "..\_shared\scaffold.ps1")
}

$mcpPython = Smash-ReadMcpPython $wikiPath
$codexConfig = Join-Path $HOME ".codex\config.toml"
if (Test-Path $codexConfig) {
    $command = $mcpPython | ConvertTo-Json -Compress
    $wiki = $wikiPath | ConvertTo-Json -Compress
    $block = "[mcp_servers.Smash]`ncommand = $command`nargs = [`"-m`", `"smash_mcp`", `"--wiki`", $wiki, `"--surface`", `"slim`"]`n"
    $text = Get-Content -Raw -Encoding UTF8 $codexConfig
    $pattern = "(?ms)^\[mcp_servers\.Smash\]\r?\n.*?(?=^\[|\z)"
    if ([regex]::IsMatch($text, $pattern)) {
        $text = [regex]::Replace($text, $pattern, $block)
        if (-not $text.EndsWith("`n")) {
            $text += "`n"
        }
    } else {
        $text = $text.TrimEnd() + "`n`n" + $block
    }
    Set-Content -Encoding UTF8 -NoNewline -Path $codexConfig -Value $text
    Write-Host "  ✓ Smash MCP registered in $codexConfig"
} else {
    Write-Host "  MCP config: add to ${codexConfig}:"
    Write-Host "  [mcp_servers.Smash]"
    Write-Host "  command = `"$mcpPython`""
    Write-Host "  args = [`"-m`", `"smash_mcp`", `"--wiki`", `"$wikiPath`", `"--surface`", `"slim`"]"
}

Smash-PrintNextSteps $mode
