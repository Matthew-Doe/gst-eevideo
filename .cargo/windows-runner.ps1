param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Command
)

function Add-PathEntry {
    param(
        [string]$Candidate
    )

    if (-not $Candidate) {
        return
    }

    $resolved = Resolve-Path -LiteralPath $Candidate -ErrorAction SilentlyContinue
    if (-not $resolved) {
        return
    }

    $resolvedPath = $resolved.Path
    $pathEntries = @($env:Path -split ';' | Where-Object { $_ })
    if ($pathEntries -contains $resolvedPath) {
        return
    }

    $env:Path = "$resolvedPath;$env:Path"
}

function Add-GStreamerBinCandidates {
    $candidates = @()

    if ($env:GSTREAMER_BIN_DIR) {
        $candidates += $env:GSTREAMER_BIN_DIR
    }

    if ($env:GSTREAMER_ROOT) {
        $candidates += Join-Path $env:GSTREAMER_ROOT "bin"
    }

    if ($env:GSTREAMER_LIB_DIR) {
        $candidates += Join-Path (Split-Path -Parent $env:GSTREAMER_LIB_DIR) "bin"
    }

    if ($env:PKG_CONFIG_PATH) {
        foreach ($entry in ($env:PKG_CONFIG_PATH -split ';' | Where-Object { $_ })) {
            $candidates += Join-Path $entry "..\..\bin"
        }
    }

    foreach ($candidate in $candidates) {
        Add-PathEntry $candidate
    }
}

Add-GStreamerBinCandidates

if ($Command.Length -eq 0) {
    Write-Error "windows-runner.ps1 expected a command to execute"
    exit 1
}

$exe = $Command[0]
$args = @()
if ($Command.Length -gt 1) {
    $args = $Command[1..($Command.Length - 1)]
}

& $exe @args
exit $LASTEXITCODE
