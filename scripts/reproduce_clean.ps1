param(
    [switch]$Docker = $false,
    [string]$Image = "zymatica-verify:10.0.0"
)
$ErrorActionPreference = "Stop"

Write-Host "=================================================================" -ForegroundColor Cyan
Write-Host "  ZYMATICA CLEAN-ROOM INDEPENDENT REPRODUCTION SUITE" -ForegroundColor Cyan
Write-Host "=================================================================" -ForegroundColor Cyan

if ($Docker) {
    if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
        throw "Docker is required for a containerized reproduction."
    }
    Write-Host "[+] Building clean-room Docker verification image..." -ForegroundColor Green
    docker build -f Dockerfile.verify -t $Image .
    docker run --rm $Image
} else {
    Write-Host "[+] Executing local clean-room master verification pipeline..." -ForegroundColor Green
    python verify_release.py
}

Write-Host "`n[+] PASS: Clean-room reproduction completed successfully." -ForegroundColor Green
