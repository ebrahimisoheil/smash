param()

$ErrorActionPreference = "Stop"

function Smash-NewParentDirectory {
    param([Parameter(Mandatory = $true)][string]$Path)

    $parent = Split-Path -Parent $Path
    if ($parent) {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }
}

function Smash-UpsertInstructions {
    param(
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string]$SourceFile,
        [Parameter(Mandatory = $true)][string]$Label
    )

    Smash-NewParentDirectory $Target
    $source = (Get-Content -Raw -Encoding UTF8 $SourceFile).TrimEnd()
    $existing = ""
    if (Test-Path $Target) {
        $existing = Get-Content -Raw -Encoding UTF8 $Target
    }

    $headers = @("## Smash — Local Agent Memory", "## Smash — Personal Knowledge Wiki")
    $headerPattern = ($headers | ForEach-Object { [regex]::Escape($_) }) -join "|"
    $pattern = "(?s)(^|`n)(?:$headerPattern)`n.*?(?=`n## |\z)"

    if ([regex]::IsMatch($existing, $pattern)) {
        $updated = [regex]::Replace($existing, $pattern, {
            param($match)
            $prefix = if ($match.Groups[1].Value) { "`n" } else { "" }
            return $prefix + $source
        }).TrimEnd() + "`n"
    } else {
        $separator = if ($existing.Trim()) { "`n`n" } else { "" }
        $updated = $existing.TrimEnd() + $separator + $source + "`n"
    }

    Set-Content -Encoding UTF8 -NoNewline -Path $Target -Value $updated
    Write-Host "$Label -> $Target"
}

function Smash-ToHashtable {
    param($InputObject)

    if ($null -eq $InputObject) {
        return @{}
    }
    if ($InputObject -is [System.Collections.IDictionary]) {
        $out = @{}
        foreach ($key in $InputObject.Keys) {
            $out[$key] = Smash-ToHashtable $InputObject[$key]
        }
        return $out
    }
    if ($InputObject -is [System.Management.Automation.PSCustomObject]) {
        $out = @{}
        foreach ($property in $InputObject.PSObject.Properties) {
            $out[$property.Name] = Smash-ToHashtable $property.Value
        }
        return $out
    }
    if ($InputObject -is [System.Array]) {
        return @($InputObject | ForEach-Object { Smash-ToHashtable $_ })
    }
    return $InputObject
}

function Smash-UpsertMcpJson {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Command,
        [Parameter(Mandatory = $true)][string]$WikiPath,
        [string]$TopKey = "mcpServers",
        [switch]$IncludeType,
        [switch]$IncludeDisabled
    )

    Smash-NewParentDirectory $Path
    $config = @{}
    if (Test-Path $Path) {
        try {
            $raw = Get-Content -Raw -Encoding UTF8 $Path
            if ($raw.Trim()) {
                $config = Smash-ToHashtable ($raw | ConvertFrom-Json)
            }
        } catch {
            Write-Host "  · Could not parse $Path; leaving it unchanged."
            Write-Host "    Add manually: $Command -m smash_mcp --wiki $WikiPath --surface slim"
            return
        }
    }

    if (-not $config.ContainsKey($TopKey) -or -not ($config[$TopKey] -is [System.Collections.IDictionary])) {
        $config[$TopKey] = @{}
    }

    $server = @{
        command = $Command
        args = @("-m", "smash_mcp", "--wiki", $WikiPath, "--surface", "slim")
    }
    if ($IncludeType) {
        $server["type"] = "stdio"
    }
    if ($IncludeDisabled) {
        $server["disabled"] = $false
    }

    $config[$TopKey]["Smash"] = $server
    $json = $config | ConvertTo-Json -Depth 20
    Set-Content -Encoding UTF8 -Path $Path -Value ($json + "`n")
    Write-Host "  ✓ Smash MCP registered in $Path"
}

function Smash-ReadMcpPython {
    param([Parameter(Mandatory = $true)][string]$WikiPath)

    $root = Split-Path -Parent $WikiPath
    $marker = Join-Path $root ".smash-mcp-python"
    if (Test-Path $marker) {
        $value = (Get-Content -Raw -Encoding UTF8 $marker).Trim()
        if ($value) {
            return $value
        }
    }
    return "py"
}

function Smash-PrintNextSteps {
    param([string]$Mode = "--global")

    Write-Host ""
    Write-Host "Done."
    if ($Mode -eq "--project") {
        Write-Host "  Drop sources into raw/."
        Write-Host "  View wiki: py smash.py serve"
        Write-Host "  Print starter prompts: py smash.py next"
        Write-Host "  Try in your agent:"
        Write-Host "    is Smash ready?"
        Write-Host "    start with Smash before we continue"
        Write-Host "    remember that this project uses Smash for local agent memory"
        Write-Host "    what does Smash remember about this project?"
        Write-Host "    ingest raw/<file> into Smash"
    } else {
        Write-Host "  Drop sources into ~/Smash/raw/."
        Write-Host "  View wiki: smash serve"
        Write-Host "  Print starter prompts: smash next"
        Write-Host "  Try in your agent:"
        Write-Host "    is Smash ready?"
        Write-Host "    start with Smash before we continue"
        Write-Host "    remember that I prefer local-first agent memory"
        Write-Host "    what does Smash know about me?"
        Write-Host "    ingest raw/<file> into Smash"
    }
}
