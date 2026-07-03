#!/usr/bin/env pwsh
# paper-trading-terminal installer for Windows
# Usage: iwr https://github.com/<owner>/paper-trading-terminal/raw/main/install.ps1 | iex

$ErrorActionPreference = 'Stop'

$repo        = if ($env:PAPER_INSTALL_REPO) { $env:PAPER_INSTALL_REPO } else { 'tsui66/paper-trading-terminal' }
$binName     = 'paper'
$packageName = 'paper-trading-terminal'
$installDir  = Join-Path $env:LOCALAPPDATA 'Programs\paper'

if ($env:PAPER_INSTALL_VERSION) {
    $version = $env:PAPER_INSTALL_VERSION
} else {
    Write-Host "Fetching latest release..."
    try {
        $req = [System.Net.HttpWebRequest]::Create("https://github.com/$repo/releases/latest")
        $req.AllowAutoRedirect = $false
        $req.Timeout = 15000
        $resp = $req.GetResponse()
        $location = $resp.Headers['Location']
        $resp.Close()
    } catch [System.Net.WebException] {
        $location = $_.Exception.Response.Headers['Location']
        if (-not $location) { throw "Failed to fetch the latest release: $_" }
    }

    $version = $location -replace '^.*/tag/', ''
    if (-not $version -or $version -eq $location) {
        throw "Failed to parse version from redirect URL: $location"
    }
}

Write-Host "Installing $packageName@$version"

$tmpDir  = Join-Path $env:TEMP ([System.IO.Path]::GetRandomFileName())
$zipPath = Join-Path $tmpDir "$binName.zip"

New-Item -ItemType Directory -Path $tmpDir | Out-Null

try {
    if ($env:PAPER_INSTALL_FROM) {
        Write-Host "Using local package: $($env:PAPER_INSTALL_FROM)"
        Copy-Item $env:PAPER_INSTALL_FROM $zipPath
    } else {
        $downloadUrl = "https://github.com/$repo/releases/download/$version/$packageName-windows-amd64.zip"
        Write-Host "Downloading $downloadUrl"
        $wc = New-Object System.Net.WebClient
        $wc.DownloadFile($downloadUrl, $zipPath)
        $wc.Dispose()
    }

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [System.IO.Compression.ZipFile]::ExtractToDirectory($zipPath, $tmpDir)

    if (-not (Test-Path $installDir)) {
        New-Item -ItemType Directory -Path $installDir | Out-Null
    }

    $srcExe  = Join-Path $tmpDir "$binName.exe"
    $destExe = Join-Path $installDir "$binName.exe"
    Move-Item -Path $srcExe -Destination $destExe -Force

} finally {
    Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
}

$userPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
if ($userPath -notlike "*$installDir*") {
    $newPath = ($userPath.TrimEnd(';') + ";$installDir").TrimStart(';')
    [Environment]::SetEnvironmentVariable('PATH', $newPath, 'User')
    Write-Host ""
    Write-Host "Added $installDir to your PATH."
    Write-Host "Restart your terminal for the PATH change to take effect."
}

Write-Host ""
Write-Host "paper CLI $version installed to $installDir\$binName.exe"
Write-Host ""
Write-Host "Next steps:"
Write-Host "  paper -h                      # verify install"
Write-Host "  paper config provider-status  # check yahoo (primary) + fcontext (optional)"
Write-Host "  paper quote AAPL              # test live quote"
Write-Host "  paper tui                     # launch dashboard"
Write-Host ""
Write-Host "Optional — fcontext fallback when Yahoo is down:"
Write-Host "  iwr https://github.com/aitaport/fcontext-cli/releases/latest/download/install.ps1 | iex"
Write-Host "  fcontext auth login"
Write-Host "  paper config provider-status"
Write-Host ""