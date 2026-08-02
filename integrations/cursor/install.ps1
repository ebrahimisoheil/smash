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
$target = if ($Project) { ".cursor\rules\Smash.mdc" } else { Join-Path $HOME ".cursor\rules\Smash.mdc" }
$wikiPath = if ($Project) { Join-Path (Get-Location).Path "wiki" } else { Join-Path $HOME "Smash\wiki" }

Smash-NewParentDirectory $target
$instructions = Get-Content -Raw -Encoding UTF8 $instructionsFile
$rule = "---`ndescription: Smash knowledge wiki context`nalwaysApply: true`n---`n`n$instructions"
Set-Content -Encoding UTF8 -Path $target -Value $rule
Write-Host "Smash rule -> $target"

if ($Project) {
    & (Join-Path $ScriptDir "..\_shared\scaffold.ps1") -Project
} else {
    & (Join-Path $ScriptDir "..\_shared\scaffold.ps1")
}

$mcpPython = Smash-ReadMcpPython $wikiPath
if (-not $Project) {
    $mcpConfig = Join-Path $HOME ".cursor\mcp.json"
    if (Test-Path $mcpConfig) {
        Smash-UpsertMcpJson -Path $mcpConfig -Command $mcpPython -WikiPath $wikiPath
    } else {
        Write-Host "  Add to ${mcpConfig}:"
        Write-Host "  { `"mcpServers`": { `"Smash`": { `"command`": `"$mcpPython`", `"args`": [`"-m`", `"smash_mcp`", `"--wiki`", `"$wikiPath`", `"--surface`", `"slim`"] } } }"
    }
}

Smash-PrintNextSteps $mode
