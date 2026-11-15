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
    function Invoke-Control([string] $Method, [hashtable] $Params = @{}) {
      $request = @{ id = 1; key = $key; method = $Method; params = $Params } | ConvertTo-Json -Depth 5 -Compress
      $writer.WriteLine($request)
      $response = $reader.ReadLine() | ConvertFrom-Json
      if (-not $response.ok) { throw "sample GUI command failed: $Method" }
      return $response.result
    }

    function Wait-ForState([string] $Description, [scriptblock] $Condition) {
      for ($i = 0; $i -lt 40; $i++) {
        $state = Invoke-Control 'ui.inspect'
        if (& $Condition $state) { return $state }
        Start-Sleep -Milliseconds 250
      }
      throw "sample GUI did not show $Description"
    }

    $null = Invoke-Control 'ui.dismissReminder' @{ all = $true }
    $null = Invoke-Control 'ui.theme' @{ value = 'light' }
    $state = Wait-ForState 'the redesigned inbox' {
      param($state)
      ($state.items.kind -contains 'listTitle') -and ($state.items.kind -contains 'addMailbox') -and ($state.items.kind -contains 'message')
    }
    $message = $state.items | Where-Object kind -eq 'message' | Select-Object -First 1
    $null = Invoke-Control 'ui.click' @{ kind = 'message'; id = $message.id }
    $null = Wait-ForState 'the selected message' { param($state) $state.selection.messages -contains $message.id }
    $null = Invoke-Control 'ui.screenshot' @{ path = "$env:RUNNER_TEMP\mailcraft-windows-sample.png" }
    $null = Invoke-Control 'ui.theme' @{ value = 'dark' }
    $null = Wait-ForState 'dark mode' { param($state) $state.ui.theme -eq 'dark' }
    $null = Invoke-Control 'ui.screenshot' @{ path = "$env:RUNNER_TEMP\mailcraft-windows-sample-dark.png" }

    $null = Invoke-Control 'ui.click' @{ kind = 'addMailbox'; id = 0 }
    $null = Wait-ForState 'the add mailbox dialog' { param($state) $state.dialog -eq 'account' }
    $null = Invoke-Control 'ui.closeDialog'
    # ribbon::hit_id("mail.new"), the same control UI automation sees in ui.inspect.
    $null = Invoke-Control 'ui.click' @{ kind = 'ribbon'; id = [UInt64]1564654267571626366 }
    $null = Wait-ForState 'the new email composer' { param($state) $state.composing }
    $null = Invoke-Control 'compose.close'
    $null = Invoke-Control 'app.quit'
    Write-Output 'ok sample GUI: inbox, selection, both themes, add mailbox, and compose'
  } finally { $tcp.Dispose() }
  if (-not $app.WaitForExit(30000)) { throw 'sample GUI did not quit' }
} finally {
  if (-not $app.HasExited) { Stop-Process -Id $app.Id -Force }
  Remove-Item Env:MAILCRAFT_NO_PREFS -ErrorAction SilentlyContinue
}
