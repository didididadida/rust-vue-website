<#
.SYNOPSIS
    Push the open-source package to GitHub.

.DESCRIPTION
    Input  -> local wanle-oss repo
    Output -> pushed to origin (rust-vue-website)
    Mechanism:
      - HTTPS + Personal Access Token (SSH key is not configured for GitHub)
      - token is read from the GITHUB_TOKEN environment variable.
        It is never written to disk, never echoed.
      - pre-flight checks: clean worktree, has commits, remote points at the repo
    Invariant: the token never appears in any file or log line.
    Edge cases: invalid token -> git returns 128; missing repo -> 128.

.BASIS
Classic problem: secret externalization (12-factor config)
Source: GitHub Docs - Managing your personal access tokens
        https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens
        Password authentication for Git over HTTPS was removed on 2021-08-13.
Prototype conclusion:
  1. Git over HTTPS accepts only a token, plain passwords are rejected.
  2. A token is a credential: keep it out of the repo, scripts and logs;
     inject it via environment variable or credential manager.
Local variant:
  - The token stored in Windows Credential Manager is stale
    (observed: "Invalid username or token").
  - The user wants a one-command retry instead of retyping a long git command.
Adopted: read the token from the environment, keep this script committable
         and re-runnable; on failure print the concrete next step.
Deviation: no token pre-validation (that needs an extra API round trip);
           git reports the error and this script translates it into guidance.
#>
param(
    [string]$Repo = 'D:\sakesoul\wanle-oss',
    [string]$Branch = 'main',
    [string]$Owner = 'didididadida',
    [string]$Name = 'rust-vue-website'
)

$ErrorActionPreference = 'Stop'

Write-Host '=== 1. Repository state ===' -ForegroundColor Cyan
if (-not (Test-Path (Join-Path $Repo '.git'))) {
    Write-Host "FAIL: $Repo is not a git repository" -ForegroundColor Red
    exit 1
}

Push-Location $Repo
try {
    $dirty = (git status --porcelain | Measure-Object).Count
    if ($dirty -gt 0) {
        Write-Host "FAIL: $dirty uncommitted change(s). Commit before pushing." -ForegroundColor Red
        exit 1
    }
    $head = git rev-parse --short HEAD
    Write-Host "  HEAD:   $head"
    Write-Host "  remote: $(git remote get-url origin)"
    Write-Host "  target: $Owner/$Name ($Branch)"

    Write-Host ''
    Write-Host '=== 2. Credential ===' -ForegroundColor Cyan
    $token = $env:GITHUB_TOKEN
    if (-not $token) {
        Write-Host 'FAIL: GITHUB_TOKEN is not set' -ForegroundColor Red
        Write-Host ''
        Write-Host 'Create a token at https://github.com/settings/tokens (scope: repo), then:' -ForegroundColor Yellow
        Write-Host '  $env:GITHUB_TOKEN = "ghp_your_token"' -ForegroundColor Yellow
        Write-Host '  .\scripts\push-oss.ps1' -ForegroundColor Yellow
        exit 1
    }
    Write-Host "  GITHUB_TOKEN: set (length $($token.Length))"

    Write-Host ''
    Write-Host '=== 3. Ensure repository exists ===' -ForegroundColor Cyan
    $apiHeaders = @{
        Authorization          = "Bearer $token"
        'User-Agent'           = 'wanle-oss-push'
        'Accept'               = 'application/vnd.github+json'
    }
    $repoUrl = "https://api.github.com/repos/$Owner/$Name"
    $exists = $false
    try {
        Invoke-RestMethod -Uri $repoUrl -Headers $apiHeaders -Method Get -TimeoutSec 20 | Out-Null
        $exists = $true
        Write-Host "  exists: $Owner/$Name"
    } catch {
        $status = $null
        if ($_.Exception.Response) { $status = [int]$_.Exception.Response.StatusCode }
        if ($status -eq 404) {
            Write-Host '  not found, creating...' -ForegroundColor Yellow
            $payload = @{
                name        = $Name
                description = 'Rust+Vue website project: personal blog system with actix-web backend and Vue 3 frontend'
                private     = $false
                has_issues  = $true
                has_wiki    = $false
                auto_init   = $false
            } | ConvertTo-Json -Compress
            $pbytes = [System.Text.Encoding]::UTF8.GetBytes($payload)
            try {
                Invoke-RestMethod -Uri 'https://api.github.com/user/repos' -Method Post -Headers $apiHeaders `
                    -ContentType 'application/json; charset=utf-8' -Body $pbytes -TimeoutSec 30 | Out-Null
                $exists = $true
                Write-Host "  created: $Owner/$Name" -ForegroundColor Green
            } catch {
                Write-Host "FAIL: cannot create repository: $($_.Exception.Message)" -ForegroundColor Red
                $msg = $null
                if ($_.ErrorDetails) { $msg = $_.ErrorDetails.Message }
                if ($msg) { Write-Host "  $msg" }
                Write-Host '  The token needs permission: Repository permissions -> Administration -> Read and write' -ForegroundColor Yellow
                exit 1
            }
        } else {
            Write-Host "FAIL: cannot query repository (HTTP $status)" -ForegroundColor Red
            exit 1
        }
    }

    Write-Host ''
    Write-Host '=== 4. Push ===' -ForegroundColor Cyan
    $authUrl = "https://x-access-token:$token@github.com/$Owner/$Name.git"
    $output = git push $authUrl "HEAD:refs/heads/$Branch" 2>&1
    $code = $LASTEXITCODE
    $output | ForEach-Object { "  $_" }

    Write-Host ''
    if ($code -eq 0) {
        Write-Host "OK: https://github.com/$Owner/$Name" -ForegroundColor Green
    } else {
        Write-Host "FAIL: push exited with $code" -ForegroundColor Red
        Write-Host 'Likely causes:' -ForegroundColor Yellow
        Write-Host '  - token invalid or expired     -> generate a new one'
        Write-Host '  - token lacks Contents write   -> Repository permissions -> Contents -> Read and write'
        Write-Host '  - branch protection on remote  -> check repository settings'
        exit 1
    }
} finally {
    Pop-Location
}
