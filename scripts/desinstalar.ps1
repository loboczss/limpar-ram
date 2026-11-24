# Remove o memlimpa: tarefas agendadas, executável e configuração. Pede admin uma vez.
$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) {
    Start-Process powershell -Verb RunAs -Wait -ArgumentList "-ExecutionPolicy Bypass -File `"$PSCommandPath`""
    exit
}

foreach ($t in 'memlimpa', 'memlimpa-tudo', 'memlimpa-bandeja') {
    Unregister-ScheduledTask -TaskName $t -Confirm:$false -ErrorAction SilentlyContinue
}
Get-Process memlimpa -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500
Remove-Item (Join-Path $env:USERPROFILE 'bin\memlimpa.exe') -Force -ErrorAction SilentlyContinue
Remove-Item (Join-Path $env:APPDATA 'memlimpa') -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item (Join-Path $env:LOCALAPPDATA 'memlimpa') -Recurse -Force -ErrorAction SilentlyContinue
"memlimpa removido."
Start-Sleep 2
