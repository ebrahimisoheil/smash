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
$target = if ($Project) { "GEMINI.md" } else { Join-Path $HOME ".gemini\GEMINI.md" }
$wikiPath = if ($Project) { Join-Path (Get-Location).Path "wiki" } else { Join-Path $HOME "Smash\wiki" }

Smash-UpsertInstructions $target $instructionsFile "Smash instructions"

if ($Project) {
    & (Join-Path $ScriptDir "..\_shared\scaffold.ps1") -Project
} else {
    & (Join-Path $ScriptDir "..\_shared\scaffold.ps1")
}

$mcpPython = Smash-ReadMcpPython $wikiPath
$settings = Join-Path $HOME ".gemini\settings.json"
if (-not $Project -and (Test-Path $settings)) {
    Smash-UpsertMcpJson -Path $settings -Command $mcpPython -WikiPath $wikiPath
} else {
    Write-Host ""
    Write-Host "  MCP: add to ${settings}:"
    Write-Host "  { `"mcpServers`": { `"Smash`": { `"command`": `"$mcpPython`", `"args`": [`"-m`", `"smash_mcp`", `"--wiki`", `"$wikiPath`", `"--surface`", `"slim`"] } } }"
}

Smash-PrintNextSteps $mode
