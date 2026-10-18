<# Sample-only smoke check of an installed app. Never opens a real mailbox. #>
param([Parameter(Mandatory = $true)] [string] $AppDir)
$ErrorActionPreference = 'Stop'
$cli = Join-Path $AppDir 'mailcraft-cli.exe'
& $cli run --sample --cmd mail.markAllRead --save "$env:RUNNER_TEMP\smoke.mailcraft"
if ($LASTEXITCODE -ne 0) { throw 'sample CLI command/save failed' }
& $cli list "$env:RUNNER_TEMP\smoke.mailcraft" --folder Inbox
if ($LASTEXITCODE -ne 0) { throw 'sample mailbox reopen failed' }

# Start the actual GUI and require an authenticated response from its sample mailbox.
$env:MAILCRAFT_NO_PREFS = '1'
$app = Start-Process (Join-Path $AppDir 'mailcraft.exe') -ArgumentList @('--sample', '--control', '17991') -PassThru
$keyFile = Join-Path $env:APPDATA 'MailCraft\control-key.local-17991'
try {
  $ready = $false
  for ($i = 0; $i -lt 60; $i++) {
    if ($app.HasExited) { throw "sample GUI exited ($($app.ExitCode))" }
    if (Test-Path $keyFile) { $ready = $true; break }
    Start-Sleep -Seconds 1
  }
  if (-not $ready) { throw 'sample GUI control channel did not start' }
  $key = (Get-Content -Raw $keyFile).Trim()
  $tcp = [Net.Sockets.TcpClient]::new('127.0.0.1', 17991)
  try {
    $tcp.ReceiveTimeout = 30000
    $reader = [IO.StreamReader]::new($tcp.GetStream())
    $writer = [IO.StreamWriter]::new($tcp.GetStream())
    $writer.AutoFlush = $true
    foreach ($method in 'ui.inspect', 'ui.screenshot', 'app.quit') {
      $params = if ($method -eq 'ui.screenshot') { @{path = "$env:RUNNER_TEMP\mailcraft-windows-sample.png"} } else { @{} }
      $request = @{ id = 1; key = $key; method = $method; params = $params } | ConvertTo-Json -Compress
      $writer.WriteLine($request)
      $response = $reader.ReadLine() | ConvertFrom-Json
      if (-not $response.ok) { throw "sample GUI command failed: $method" }
      Write-Output "ok sample GUI: $method"
    }
  } finally { $tcp.Dispose() }
  if (-not $app.WaitForExit(30000)) { throw 'sample GUI did not quit' }
} finally {
  if (-not $app.HasExited) { Stop-Process -Id $app.Id -Force }
  Remove-Item Env:MAILCRAFT_NO_PREFS -ErrorAction SilentlyContinue
}
