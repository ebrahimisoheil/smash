param([switch]$Project)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $PSCommandPath
$SmashRoot = (Resolve-Path (Join-Path $ScriptDir "..\..")).Path
$Mode = if ($Project) { "--project" } else { "--global" }
$TargetDir = if ($Project) { (Get-Location).Path } else { Join-Path $HOME "Smash" }
$BasePython = if (Get-Command py -ErrorAction SilentlyContinue) {
    "py"
} elseif (Get-Command python -ErrorAction SilentlyContinue) {
    "python"
} else {
    throw "Python was not found. Install Python 3 and rerun this installer."
}

if (-not $Project) {
    New-Item -ItemType Directory -Force -Path $TargetDir | Out-Null
}

function Copy-LinkFile {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination,
        [string]$Label = ""
    )

    if (Test-Path $Source) {
        $parent = Split-Path -Parent $Destination
        if ($parent) {
            New-Item -ItemType Directory -Force -Path $parent | Out-Null
        }
        Copy-Item -Force -Path $Source -Destination $Destination
        if ($Label) {
            Write-Host "  Updated $Label"
        }
    }
}

function Install-LinkCommandWrapper {
    if ($Project -or -not (Test-Path (Join-Path $TargetDir "smash.py"))) {
        return
    }

    $cliDir = if ($env:SMASH_CLI_DIR) { $env:SMASH_CLI_DIR } else { Join-Path $HOME ".local\bin" }
    $cmdPath = Join-Path $cliDir "smash.cmd"
    $psPath = Join-Path $cliDir "smash.ps1"
    $legacyCmdPath = Join-Path $cliDir "Smash.cmd"
    $legacyPsPath = Join-Path $cliDir "Smash.ps1"
    $marker = "Smash command wrapper"
    $linkPy = Join-Path $TargetDir "smash.py"

    New-Item -ItemType Directory -Force -Path $cliDir | Out-Null

    foreach ($legacyPath in @($legacyCmdPath, $legacyPsPath)) {
        if ((Test-Path $legacyPath) -and (Select-String -Quiet -SimpleMatch $marker $legacyPath)) {
            Remove-Item -Force $legacyPath
            Write-Host "  Removed old Smash wrapper: $legacyPath"
        }
    }

    if ((Test-Path $cmdPath) -and -not (Select-String -Quiet -SimpleMatch $marker $cmdPath)) {
        Write-Host "  · $cmdPath already exists and is not a Smash wrapper; not overwriting."
        Write-Host "    Fallback: $BasePython `"$linkPy`" health"
        return
    }

    $cmd = @"
@echo off
REM $marker
set SMASH_CLI_COMMAND=smash
$BasePython "$linkPy" %*
"@
    Set-Content -Encoding ASCII -Path $cmdPath -Value $cmd

    $ps = @"
# $marker
$env:SMASH_CLI_COMMAND = "smash"
& $BasePython "$linkPy" @args
exit `$LASTEXITCODE
"@
    Set-Content -Encoding UTF8 -Path $psPath -Value $ps

    Write-Host "  ✓ Smash command: $cmdPath"
    $pathParts = ($env:PATH -split [IO.Path]::PathSeparator)
    if ($pathParts -notcontains $cliDir) {
        Write-Host "  · Add $cliDir to PATH to run: smash health"
    }
}

$isUpdate = (Test-Path (Join-Path $TargetDir "wiki\index.md")) -or (Test-Path (Join-Path $TargetDir "wiki\log.md"))
if ($isUpdate) {
    Write-Host "  Existing wiki detected at $TargetDir - updating code only, wiki data untouched."
} else {
    Write-Host "  Fresh install at $TargetDir."
}

Copy-LinkFile (Join-Path $SmashRoot "serve.py") (Join-Path $TargetDir "serve.py") "serve.py"
Copy-LinkFile (Join-Path $SmashRoot "SMASH.md") (Join-Path $TargetDir "SMASH.md") "SMASH.md"
Copy-LinkFile (Join-Path $SmashRoot "smash.py") (Join-Path $TargetDir "smash.py") "smash.py"
Copy-LinkFile (Join-Path $SmashRoot "logo.png") (Join-Path $TargetDir "logo.png")
Copy-LinkFile (Join-Path $SmashRoot "logo.svg") (Join-Path $TargetDir "logo.svg")
Copy-LinkFile (Join-Path $SmashRoot ".smashignore") (Join-Path $TargetDir ".smashignore")

$coreDir = Join-Path $SmashRoot "mcp_package\smash_core"
if (Test-Path $coreDir) {
    $targetCore = Join-Path $TargetDir "smash_core"
    New-Item -ItemType Directory -Force -Path $targetCore | Out-Null
    Copy-Item -Force -Path (Join-Path $coreDir "*.py") -Destination $targetCore
    Write-Host "  Updated smash_core"
}

$dirs = @(
    "raw",
    "wiki\sources",
    "wiki\concepts",
    "wiki\entities",
    "wiki\memories",
    "wiki\comparisons",
    "wiki\explorations"
)
foreach ($dir in $dirs) {
    $path = Join-Path $TargetDir $dir
    New-Item -ItemType Directory -Force -Path $path | Out-Null
    if (-not $isUpdate) {
        New-Item -ItemType File -Force -Path (Join-Path $path ".gitkeep") | Out-Null
    }
}

if (-not $isUpdate) {
    & $BasePython (Join-Path $TargetDir "smash.py") doctor --fix $TargetDir *> $null
    if ($LASTEXITCODE -ne 0) {
        throw "Smash wiki initialization failed."
    }
    Write-Host "  Wiki structure created at $TargetDir"
}

Write-Host "  Wiki ready at $TargetDir"
Install-LinkCommandWrapper

Write-Host ""
Write-Host "  Setting up MCP server..."

$linkMcpPackage = if (Test-Path (Join-Path $SmashRoot "mcp_package")) {
    Join-Path $SmashRoot "mcp_package"
} else {
    "smash-mcp"
}
$mcpPython = $BasePython
$venv = if ($env:SMASH_MCP_VENV) { $env:SMASH_MCP_VENV } else { Join-Path $HOME ".smash-mcp-venv" }
$venvPython = Join-Path $venv "Scripts\python.exe"
$marker = Join-Path $TargetDir ".smash-mcp-python"
$installed = $false
$reused = $false

& $BasePython -m pip install --upgrade $linkMcpPackage -q *> $null
if ($LASTEXITCODE -eq 0) {
    $installed = $true
    $mcpPython = $BasePython
} else {
    & $BasePython -m venv $venv *> $null
    if ($LASTEXITCODE -eq 0 -and (Test-Path $venvPython)) {
        & $venvPython -m pip install --upgrade pip -q *> $null
        if ($LASTEXITCODE -eq 0) {
            & $venvPython -m pip install --upgrade $linkMcpPackage -q *> $null
            if ($LASTEXITCODE -eq 0) {
                $installed = $true
                $mcpPython = $venvPython
            }
        }
    }
}

if (-not $installed -and (Test-Path $marker)) {
    $candidate = (Get-Content -Raw -Encoding UTF8 $marker).Trim()
    if ($candidate) {
        & $candidate -c "import smash_mcp" *> $null
        if ($LASTEXITCODE -eq 0) {
            $installed = $true
            $reused = $true
            $mcpPython = $candidate
        }
    }
} elseif (-not $installed -and (Test-Path $venvPython)) {
    & $venvPython -c "import smash_mcp" *> $null
    if ($LASTEXITCODE -eq 0) {
        $installed = $true
        $reused = $true
        $mcpPython = $venvPython
    }
}

if ($installed) {
    Set-Content -Encoding UTF8 -Path $marker -Value ($mcpPython + "`n")
    if ($reused) {
        Write-Host "  ✓ existing smash-mcp available"
        Write-Host "  · Automatic upgrade did not complete; run verify-mcp to confirm the installed version."
    } else {
        Write-Host "  ✓ smash-mcp installed"
    }
    if ($mcpPython -ne $BasePython) {
        Write-Host "  ✓ MCP Python: $mcpPython"
    }
    Write-Host ""
    Write-Host "  Add to your MCP client config:"
    Write-Host "  {"
    Write-Host "    `"mcpServers`": {"
    Write-Host "      `"Smash`": {"
    Write-Host "        `"command`": `"$mcpPython`","
    Write-Host "        `"args`": [`"-m`", `"smash_mcp`", `"--wiki`", `"$TargetDir\wiki`", `"--surface`", `"slim`"]"
    Write-Host "      }"
    Write-Host "    }"
    Write-Host "  }"
} else {
    Write-Host "  · Could not install smash-mcp automatically."
    Write-Host "  Manual options:"
    Write-Host "    $BasePython -m pip install --upgrade smash-mcp"
    Write-Host "    $BasePython -m venv ~/.smash-mcp-venv"
    Write-Host "    ~\.smash-mcp-venv\Scripts\python.exe -m pip install --upgrade pip smash-mcp"
}

if (Test-Path (Join-Path $TargetDir "smash.py")) {
    Write-Host ""
    if ($Project) {
        Write-Host "  Check Smash readiness:"
        Write-Host "    py smash.py health"
        Write-Host "  Verify MCP setup:"
        Write-Host "    py smash.py verify-mcp"
    } else {
        Write-Host "  Check Smash readiness:"
        Write-Host "    smash health"
        Write-Host "  Verify MCP setup:"
        Write-Host "    smash verify-mcp"
    }
}
