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
$target = if ($Project) { "CLAUDE.md" } else { Join-Path $HOME ".claude\CLAUDE.md" }
$wikiPath = if ($Project) { Join-Path (Get-Location).Path "wiki" } else { Join-Path $HOME "Smash\wiki" }

Smash-UpsertInstructions $target $instructionsFile "Smash steering"

if ($Project) {
    & (Join-Path $ScriptDir "..\_shared\scaffold.ps1") -Project
} else {
    & (Join-Path $ScriptDir "..\_shared\scaffold.ps1")
}

$mcpPython = Smash-ReadMcpPython $wikiPath
$mcpConfig = Join-Path $HOME ".claude.json"
if (Test-Path $mcpConfig) {
    Smash-UpsertMcpJson -Path $mcpConfig -Command $mcpPython -WikiPath $wikiPath
} else {
    Write-Host ""
    Write-Host "  MCP config: add to $mcpConfig or .mcp.json at project root:"
    Write-Host "  { `"mcpServers`": { `"Smash`": { `"command`": `"$mcpPython`", `"args`": [`"-m`", `"smash_mcp`", `"--wiki`", `"$wikiPath`", `"--surface`", `"slim`"] } } }"
}

Smash-PrintNextSteps $mode
