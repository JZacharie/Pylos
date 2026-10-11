# =============================================================================
# build-windows.ps1 — Script de compilation et packaging Windows pour Pylos
#
# Permet de :
#   1. Compiler l'interface web React (ui/dist)
#   2. Compiler le binaire natif Windows (target/release/pylos.exe)
#   3. Créer un installeur Windows MSI (target/installer/Pylos.msi) via WiX Toolset
#   4. Lancer Pylos directement en local
#
# Exemples :
#   .\build-windows.ps1                    # Compile le binaire release
#   .\build-windows.ps1 -Installer         # Compile le binaire + package l'installeur MSI
#   .\build-windows.ps1 -Run               # Compile et lance immédiatement Pylos
#   .\build-windows.ps1 -Run -Port 3000    # Lance avec un port personnalisé
# =============================================================================

Param(
    [switch]$Installer,
    [switch]$SkipUI,
    [switch]$Run,
    [int]$Port = 0,
    [string]$Version = "0.1.0"
)

$ErrorActionPreference = "Stop"

Write-Host "=================================================" -ForegroundColor Cyan
Write-Host "   Pylos — Construction Native Windows            " -ForegroundColor Cyan
Write-Host "=================================================" -ForegroundColor Cyan

# 1. Compilation de l'interface UI (si non ignorée)
if (-not $SkipUI) {
    if (Test-Path "ui\package.json") {
        Write-Host "`n[1/3] Construction de l'interface React (ui)..." -ForegroundColor Yellow
        $npmCmd = Get-Command npm -ErrorAction SilentlyContinue
        if ($npmCmd) {
            npm --prefix ui run build
            if ($LASTEXITCODE -ne 0) {
                Write-Host "[WARN] Échec de compilation UI, continuation avec assets existants." -ForegroundColor DarkYellow
            } else {
                Write-Host "[OK] Interface UI compilée dans ui/dist." -ForegroundColor Green
            }
        } else {
            Write-Host "[WARN] npm non trouvé, skipping build UI." -ForegroundColor DarkYellow
        }
    }
} else {
    Write-Host "`n[1/3] Étape UI ignorée (-SkipUI)." -ForegroundColor DarkGray
}

# 2. Compilation du binaire Rust
Write-Host "`n[2/3] Compilation du binaire Rust (target/release/pylos.exe)..." -ForegroundColor Yellow
$cargoCmd = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $cargoCmd) {
    Write-Error "Cargo est introuvable. Veuillez installer Rust depuis https://rustup.rs"
}

cargo build --release -p pylos-server
if ($LASTEXITCODE -ne 0) {
    Write-Error "Échec de compilation de pylos-server."
}

$binaryPath = "target\release\pylos.exe"
if (-not (Test-Path $binaryPath)) {
    Write-Error "Le binaire $binaryPath n'a pas été généré."
}
Write-Host "[OK] Binaire Windows généré : $binaryPath" -ForegroundColor Green

# 3. Création de l'installateur MSI (WiX Toolset)
if ($Installer) {
    Write-Host "`n[3/3] Création de l'installateur Windows MSI..." -ForegroundColor Yellow
    $wixCmd = Get-Command wix -ErrorAction SilentlyContinue
    if (-not $wixCmd) {
        Write-Error "WiX Toolset v4+ est introuvable. Installez-le avec : dotnet tool install --global wix"
    }

    # Accepter l'EULA WiX v7 si nécessaire
    wix eula accept wix7 2>$null

    $installerDir = "target\installer"
    New-Item -ItemType Directory -Force -Path $installerDir | Out-Null
    $msiOutput = "$installerDir\Pylos.msi"

    wix build `
        -d version=$Version `
        -d PylosBinary=$binaryPath `
        -o $msiOutput `
        installer\Pylos.wxs

    if ($LASTEXITCODE -ne 0) {
        Write-Error "Échec de création du package MSI."
    }

    $msiSize = (Get-Item $msiOutput).Length / 1MB
    Write-Host ("[OK] Installeur MSI généré : {0} ({1:N2} Mo)" -f $msiOutput, $msiSize) -ForegroundColor Green
} else {
    Write-Host "`n[3/3] Installeur non demandé (ajoutez -Installer pour créer le package .msi)." -ForegroundColor DarkGray
}

# 4. Lancement optionnel
if ($Run) {
    Write-Host "`n--- Démarrage de Pylos ---" -ForegroundColor Cyan
    if ($Port -gt 0) {
        $env:PORT = "$Port"
    }
    Write-Host "Chargement de la configuration et démarrage du serveur..." -ForegroundColor Green
    & $binaryPath
} else {
    Write-Host "`nTerminé avec succès !" -ForegroundColor Green
    Write-Host "Pour exécuter Pylos directement :" -ForegroundColor White
    Write-Host "  .\$binaryPath" -ForegroundColor Gray
    Write-Host "Ou avec Docker Compose :" -ForegroundColor White
    Write-Host "  docker compose up -d" -ForegroundColor Gray
}
