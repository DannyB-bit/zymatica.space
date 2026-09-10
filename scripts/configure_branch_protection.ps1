param(
    [string]$Repository = "DannyB-bit/zymatica.space",
    [string]$Branch = "main",
    [switch]$RequireSignedCommits
)

$ErrorActionPreference = "Stop"
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    throw "GitHub CLI (gh) is required. Install it and run 'gh auth login' first."
}

gh auth status | Out-Host

$contexts = @(
    "Multi-Architecture Native Build & Test (ubuntu-latest)",
    "Multi-Architecture Native Build & Test (macos-latest)",
    "Multi-Architecture Native Build & Test (windows-latest)"
)

$body = @{
    required_status_checks = @{
        strict = $true
        contexts = $contexts
    }
    enforce_admins = $true
    required_pull_request_reviews = $null
    restrictions = $null
    required_linear_history = $true
    allow_force_pushes = $false
    allow_deletions = $false
    block_creations = $false
    required_conversation_resolution = $true
    lock_branch = $false
    allow_fork_syncing = $true
} | ConvertTo-Json -Depth 8

$body | gh api --method PUT "repos/$Repository/branches/$Branch/protection" --input - | Out-Host

if ($RequireSignedCommits) {
    gh api --method POST "repos/$Repository/branches/$Branch/protection/required_signatures" | Out-Host
}

Write-Host "PASS: protection applied to $Repository/$Branch"
Write-Host "Required checks:"
$contexts | ForEach-Object { Write-Host "  $_" }
