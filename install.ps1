$ErrorActionPreference = "Stop"

$Repo = "Zoranner/memo-cli"
$InstallDir = if ($env:MEMO_INSTALL_DIR) { $env:MEMO_INSTALL_DIR } else { Join-Path $HOME ".local\bin" }
$RequestedVersion = $env:MEMO_VERSION

function Get-TargetTriple {
    $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    switch ($arch) {
        "X64" { return "x86_64-pc-windows-msvc" }
        default { throw "Unsupported Windows architecture: $arch" }
    }
}

function Get-DownloadUrl([string]$AssetName) {
    if ($RequestedVersion) {
        return "https://github.com/$Repo/releases/download/$RequestedVersion/$AssetName"
    }
    return "https://github.com/$Repo/releases/latest/download/$AssetName"
}

function Ensure-UserPath([string]$Directory) {
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $parts = @()
    if ($userPath) {
        $parts = $userPath.Split(';', [System.StringSplitOptions]::RemoveEmptyEntries)
    }

    if ($parts -notcontains $Directory) {
        $newPath = if ($userPath) { "$userPath;$Directory" } else { $Directory }
        [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    }

    if (($env:Path.Split(';', [System.StringSplitOptions]::RemoveEmptyEntries)) -notcontains $Directory) {
        $env:Path = "$Directory;$env:Path"
    }
}

$target = Get-TargetTriple
$asset = "memo-$target.zip"
$url = Get-DownloadUrl $asset

$tempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("memo-install-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $tempDir | Out-Null

try {
    $archivePath = Join-Path $tempDir $asset
    Write-Host "Downloading $asset from $url"
    Invoke-WebRequest -Uri $url -OutFile $archivePath

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Expand-Archive -LiteralPath $archivePath -DestinationPath $tempDir -Force

    $sourceBinary = Join-Path $tempDir "memo.exe"
    $targetBinary = Join-Path $InstallDir "memo.exe"
    Copy-Item -LiteralPath $sourceBinary -Destination $targetBinary -Force

    Ensure-UserPath $InstallDir

    Write-Host "Installed memo to $targetBinary"
    Write-Host "Restart PowerShell if 'memo' is not yet available in PATH."
    Write-Host "Then run: memo awaken"
}
finally {
    Remove-Item -LiteralPath $tempDir -Recurse -Force -ErrorAction SilentlyContinue
}
