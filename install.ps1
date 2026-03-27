param(
    [string]$Version = "latest",
    [string]$InstallPath = "$env:LOCALAPPDATA\Memento"
)

$ErrorActionPreference = "Stop"

$TempDir = Join-Path $env:TEMP "memento-install-$(Get-Random)"

function Main {
    if ($Version -eq "latest") {
        $TagUrl = "https://api.github.com/repos/dagnele/memento/releases/latest"
        $Version = (Invoke-RestMethod -Uri $TagUrl -UseBasicParsing).tag_name
    }

    Write-Host "Installing Memento $Version to $InstallPath..." -ForegroundColor Cyan

    New-Item -ItemType Directory -Path $InstallPath -Force | Out-Null

    $AssetName = "memento-$Version-x86_64-pc-windows-msvc.zip"
    $DownloadUrl = "https://github.com/dagnele/memento/releases/download/$Version/$AssetName"
    $ZipPath = Join-Path $TempDir $AssetName
    $ExtractPath = Join-Path $TempDir "extracted"

    Write-Host "Downloading from $DownloadUrl..." -ForegroundColor Yellow
    New-Item -ItemType Directory -Path $TempDir -Force | Out-Null
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath -UseBasicParsing

    Write-Host "Extracting..." -ForegroundColor Yellow
    Expand-Archive -Path $ZipPath -DestinationPath $ExtractPath -Force

    $StagingDir = "memento-$Version-x86_64-pc-windows-msvc"
    $ExePath = Join-Path $ExtractPath "$StagingDir\memento.exe"
    Copy-Item -Path $ExePath -Destination $InstallPath -Force

    $MementoBinPath = $InstallPath

    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($UserPath -notlike "*$MementoBinPath*") {
        Write-Host "Adding to PATH..." -ForegroundColor Yellow
        $NewPath = if ($UserPath) { "$UserPath;$MementoBinPath" } else { $MementoBinPath }
        [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
        $env:Path = "$env:Path;$MementoBinPath"
    } else {
        Write-Host "Already in PATH" -ForegroundColor Green
    }

    Write-Host "Cleaning up temp files..." -ForegroundColor Yellow
    Remove-Item -Path $TempDir -Recurse -Force -ErrorAction SilentlyContinue

    Write-Host "Memento installed successfully!" -ForegroundColor Green
    Write-Host "Run 'memento --help' to verify."
}

$global:LASTEXITCODE = 0
trap {
    Write-Host "Error: $_" -ForegroundColor Red
    Remove-Item -Path $TempDir -Recurse -Force -ErrorAction SilentlyContinue
    exit 1
}

Main
