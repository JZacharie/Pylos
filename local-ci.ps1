# ============================================================
# local-ci.ps1
# Execute les memes checks que la pipeline GitHub Actions,
# mais en local pour un feedback plus rapide (sous Windows/PowerShell).
#
# Usage :
#   .\local-ci.ps1
#   .\local-ci.ps1 -Release
# ============================================================

Param(
    [switch]$Release
)

# Configuration de la console
$Host.UI.RawUI.WindowTitle = "Local CI - Pylos"

$PASS = 0
$FAIL = 0

function Write-Border {
    Write-Host "=======================================" -ForegroundColor Yellow
}

function Write-Header {
    param($title)
    Write-Host "--- [$title] ---" -ForegroundColor Yellow
}

function Check-Command {
    param($Name, $ScriptBlock)
    Write-Header $Name
    
    # Execute le script block et laisse la sortie s'afficher en temps reel
    & $ScriptBlock
    
    # Si la commande est externe, on verifie $LASTEXITCODE. Sinon on verifie $?
    $status = $false
    if ($LASTEXITCODE -ne $null) {
        if ($LASTEXITCODE -eq 0) { $status = $true }
    } else {
        if ($?) { $status = $true }
    }

    if ($status) {
        Write-Host "[OK] $Name passed`n" -ForegroundColor Green
        $script:PASS++
    } else {
        Write-Host "[FAIL] $Name failed`n" -ForegroundColor Red
        $script:FAIL++
    }
    # Reset exit code pour la prochaine commande
    $global:LASTEXITCODE = $null
}

Write-Border
Write-Host "  Local CI - Pylos (Windows)" -ForegroundColor Yellow
Write-Host "  $(Get-Date)" -ForegroundColor Yellow
Write-Border
Write-Host ""

# Arrete toutes les instances en cours de pylos.exe pour liberer le verrou sur le binaire
Write-Host "Arrêt de toutes les instances de pylos.exe..." -ForegroundColor Yellow
taskkill /F /IM pylos.exe 2>$null
Start-Sleep -Seconds 1

# --- Obligatoires ---
Check-Command "cargo fmt" { cargo fmt --all --check }
Check-Command "cargo clippy" { cargo clippy --workspace --all-targets -- -D warnings }

if (Get-Command cargo-nextest -ErrorAction SilentlyContinue) {
    Check-Command "cargo nextest" { cargo nextest run --workspace --no-fail-fast }
} else {
    Check-Command "cargo test" { cargo test --workspace }
}

# Compilation du binaire (Debug par défaut, ou Release)
if ($Release) {
    Check-Command "cargo build --release" { cargo build --release }
    $binaryPath = "target\release\pylos.exe"
} else {
    Check-Command "cargo build" { cargo build }
    $binaryPath = "target\debug\pylos.exe"
}

# Signature du binaire avec un certificat auto-signé
if (Test-Path $binaryPath) {
    Write-Host "Signature du binaire avec un certificat auto-signé..." -ForegroundColor Yellow
    $cert = Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -eq "CN=PylosCodeSign" } | Select-Object -First 1
    if (-not $cert) {
        $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject "CN=PylosCodeSign" -CertStoreLocation Cert:\CurrentUser\My
        $certBytes = $cert.Export([System.Security.Cryptography.X509Certificates.X509ContentType]::Cert)
        $tempCertPath = [System.IO.Path]::GetTempFileName()
        [System.IO.File]::WriteAllBytes($tempCertPath, $certBytes)
        # Import dans Trusted Root pour valider la confiance de la signature
        Import-Certificate -FilePath $tempCertPath -CertStoreLocation Cert:\CurrentUser\Root *>$null
        Remove-Item $tempCertPath
    }
    Set-AuthenticodeSignature -FilePath $binaryPath -Certificate $cert | Out-Null
    Write-Host "[OK] Signature de pylos.exe effectuée`n" -ForegroundColor Green
}

# --- Resume ---
Write-Border
Write-Host "  Resume : " -NoNewline -ForegroundColor Yellow
Write-Host "$PASS passed" -NoNewline -ForegroundColor Green
Write-Host ", " -NoNewline
Write-Host "$FAIL failed" -ForegroundColor Red
if (Test-Path $binaryPath) {
    $resolvedPath = Resolve-Path $binaryPath
    Write-Host "  Binaire disponible ici : " -NoNewline -ForegroundColor Yellow
    Write-Host $resolvedPath.Path -ForegroundColor Cyan
}
Write-Border

if ($FAIL -gt 0) {
    exit 1
}
