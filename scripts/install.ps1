#Requires -Version 5.1
<#
.SYNOPSIS
  Install vgen and vgen-mcp from an extracted release archive (Windows).

.PARAMETER Scope
  User (default) or Machine. Machine requires an elevated PowerShell session.

.PARAMETER InstallDir
  Root install directory. Default: $env:LOCALAPPDATA\Programs\ResMate

.PARAMETER DryRun
  Print actions without copying files.
#>
param(
    [ValidateSet('User', 'Machine')]
    [string] $Scope = 'User',

    [string] $InstallDir = '',

    [switch] $DryRun
)

$ErrorActionPreference = 'Stop'

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ResmateExe = Join-Path $ScriptDir 'bin\vgen.exe'
$McpExe = Join-Path $ScriptDir 'bin\vgen-mcp.exe'
$SourceTemplates = Join-Path $ScriptDir 'share\vgen\templates'

if (-not (Test-Path $ResmateExe)) {
    Write-Error "Run install.ps1 from the extracted release root (expected .\bin\vgen.exe)."
}

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    if ($Scope -eq 'Machine') {
        $InstallDir = Join-Path ${env:ProgramFiles} 'ResMate'
    } else {
        $InstallDir = Join-Path $env:LOCALAPPDATA 'Programs\ResMate'
    }
}

$BinDir = Join-Path $InstallDir 'bin'
$TemplateDir = Join-Path $InstallDir 'share\templates'

function Invoke-InstallStep {
    param([scriptblock] $Action, [string] $Description)
    if ($DryRun) {
        Write-Host "[dry-run] $Description"
    } else {
        & $Action
    }
}

# Browsers mark downloaded files with Zone.Identifier (Mark of the Web). Unblock-File
# removes that ADS so PowerShell and Windows do not treat the exe as untrusted internet content.
function Unblock-ReleaseFile {
    param([string] $Path)
    if ($DryRun) {
        Write-Host "[dry-run] Unblock-File $Path"
        return
    }
    if (Test-Path $Path) {
        Unblock-File -LiteralPath $Path -ErrorAction SilentlyContinue
    }
}

Write-Host "Installing ResMate CLI"
Write-Host "  binaries  -> $BinDir"
Write-Host "  templates -> $TemplateDir"
Write-Host "  scope     -> $Scope"

Invoke-InstallStep { New-Item -ItemType Directory -Force -Path $BinDir, $TemplateDir | Out-Null } 'create directories'

Unblock-ReleaseFile $ResmateExe
Unblock-ReleaseFile $McpExe

Invoke-InstallStep {
    Copy-Item -Path $ResmateExe, $McpExe -Destination $BinDir -Force
} 'copy binaries'

$InstalledResmate = Join-Path $BinDir 'vgen.exe'
$InstalledMcp = Join-Path $BinDir 'vgen-mcp.exe'
Unblock-ReleaseFile $InstalledResmate
Unblock-ReleaseFile $InstalledMcp

if (-not (Test-Path $SourceTemplates)) {
    Write-Error "Release bundle missing share\vgen\templates\"
}

Invoke-InstallStep {
    Copy-Item -Path (Join-Path $SourceTemplates '*') -Destination $TemplateDir -Recurse -Force
} 'copy templates'

$checks = @(
    (Join-Path $TemplateDir 'workspace'),
    (Join-Path $TemplateDir 'recipes'),
    (Join-Path $TemplateDir 'seed\vgen.yaml.tmpl')
)
foreach ($path in $checks) {
    if (-not (Test-Path $path)) {
        Write-Error "Template tree incomplete after copy (missing $path)."
    }
}

Invoke-InstallStep {
    [Environment]::SetEnvironmentVariable('VGEN_TEMPLATES_DIR', $TemplateDir, $Scope)
} "set VGEN_TEMPLATES_DIR ($Scope)"

Invoke-InstallStep {
    $pathName = [Environment]::GetEnvironmentVariable('Path', $Scope)
    if ([string]::IsNullOrEmpty($pathName)) {
        $pathName = $BinDir
    } elseif ($pathName -notlike "*$BinDir*") {
        $pathName = "$BinDir;$pathName"
    }
    [Environment]::SetEnvironmentVariable('Path', $pathName, $Scope)
} "prepend $BinDir to PATH ($Scope)"

Write-Host @"

ResMate installed successfully.

Next steps:
  1. Open a NEW terminal (PATH / env vars refresh)
  2. Verify:  vgen --help
  3. Bootstrap:
       mkdir $HOME\my-use-case; cd $HOME\my-use-case
       vgen init --name my-use-case

MCP binary: $(Join-Path $BinDir 'vgen-mcp.exe')
See docs\QUICKSTART.md and docs\mcp-setup-snippet.json in this archive.

"@
