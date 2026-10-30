$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$tauriCli = Join-Path $repoRoot 'node_modules\.bin\tauri.cmd'
$tauriArgs = @($args)

if (-not (Test-Path -LiteralPath $tauriCli)) {
  throw "Tauri CLI is missing: $tauriCli"
}

if ($tauriArgs.Count -eq 0 -or $tauriArgs[0] -ne 'dev') {
  & $tauriCli @tauriArgs
  exit $LASTEXITCODE
}

$logPath = Join-Path $repoRoot 'logs\tauri-dev.log'
$forwardedArgs = [System.Collections.Generic.List[string]]::new()
for ($i = 0; $i -lt $tauriArgs.Count; $i++) {
  if ($tauriArgs[$i] -eq '--pipe-log') {
    if ($i + 1 -lt $tauriArgs.Count -and -not $tauriArgs[$i + 1].StartsWith('--')) {
      $requestedPath = $tauriArgs[++$i]
      $logPath = if ([System.IO.Path]::IsPathRooted($requestedPath)) {
        $requestedPath
      } else {
        Join-Path $repoRoot $requestedPath
      }
    }
    continue
  }
  $forwardedArgs.Add($tauriArgs[$i])
}

$logDirectory = Split-Path -Parent $logPath
New-Item -ItemType Directory -Path $logDirectory -Force | Out-Null
"--- tauri dev $(Get-Date -Format o) ---" | Tee-Object -FilePath $logPath -Append
& $tauriCli @forwardedArgs 2>&1 | Tee-Object -FilePath $logPath -Append
exit $LASTEXITCODE
