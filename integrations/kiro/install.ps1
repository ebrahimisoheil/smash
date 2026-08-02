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
$target = if ($Project) { ".kiro\steering\Smash.md" } else { Join-Path $HOME ".kiro\steering\Smash.md" }
$wikiPath = if ($Project) { Join-Path (Get-Location).Path "wiki" } else { Join-Path $HOME "Smash\wiki" }

Smash-NewParentDirectory $target
Copy-Item -Force -Path $instructionsFile -Destination $target
Write-Host "Smash steering -> $target"

if ($Project) {
    & (Join-Path $ScriptDir "..\_shared\scaffold.ps1") -Project
} else {
    & (Join-Path $ScriptDir "..\_shared\scaffold.ps1")
}

$mcpPython = Smash-ReadMcpPython $wikiPath
if (-not $Project) {
    $mcpConfig = Join-Path $HOME ".kiro\settings\mcp.json"
    if (Test-Path $mcpConfig) {
        Smash-UpsertMcpJson -Path $mcpConfig -Command $mcpPython -WikiPath $wikiPath -IncludeDisabled
    } else {
        Write-Host "  MCP config: add to ${mcpConfig}:"
        Write-Host "  { `"mcpServers`": { `"Smash`": { `"command`": `"$mcpPython`", `"args`": [`"-m`", `"smash_mcp`", `"--wiki`", `"$wikiPath`", `"--surface`", `"slim`"], `"disabled`": false } } }"
    }
}

Smash-PrintNextSteps $mode
