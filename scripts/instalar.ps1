# Instala o memlimpa para o usuário atual (pede permissão de administrador uma vez):
#   1. copia o memlimpa.exe para %USERPROFILE%\bin
#   2. cria as tarefas agendadas que rodam elevadas sem a janela do UAC:
#        memlimpa          -> schtasks /run /tn memlimpa        (limpeza padrão)
#        memlimpa-tudo     -> schtasks /run /tn memlimpa-tudo   (limpeza completa)
#        memlimpa-bandeja  -> ícone na bandeja, abre no login
#   3. abre o ícone na bandeja
#
# Uso:  powershell -ExecutionPolicy Bypass -File scripts\instalar.ps1 [-Exe caminho\memlimpa.exe]
param([string]$Exe = (Join-Path $PSScriptRoot '..\target\release\memlimpa.exe'))
$ErrorActionPreference = 'Stop'

$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) {
    $origem = (Resolve-Path $Exe).Path
    Start-Process powershell -Verb RunAs -Wait -ArgumentList "-ExecutionPolicy Bypass -File `"$PSCommandPath`" -Exe `"$origem`""
    exit
}

$pasta = Join-Path $env:USERPROFILE 'bin'
$destino = Join-Path $pasta 'memlimpa.exe'
New-Item -ItemType Directory -Force $pasta | Out-Null
Get-Process memlimpa -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500
Copy-Item $Exe $destino -Force
"copiado: $destino"

# Coloca %USERPROFILE%\bin no PATH do usuário, se ainda não estiver.
$path = [Environment]::GetEnvironmentVariable('Path', 'User')
if (($path -split ';') -notcontains $pasta) {
    [Environment]::SetEnvironmentVariable('Path', "$path;$pasta", 'User')
    "adicionado ao PATH: $pasta (abra um terminal novo)"
}

$usuario = "$env:USERDOMAIN\$env:USERNAME"
$principal = New-ScheduledTaskPrincipal -UserId $usuario -LogonType Interactive -RunLevel Highest
$curta = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 5)

foreach ($t in @(@{ Nome = 'memlimpa'; Args = '--limpar' }, @{ Nome = 'memlimpa-tudo'; Args = '--limpar tudo' })) {
    $acao = New-ScheduledTaskAction -Execute $destino -Argument $t.Args
    Register-ScheduledTask -TaskName $t.Nome -Action $acao -Principal $principal -Settings $curta -Force | Out-Null
    "tarefa criada: $($t.Nome)"
}

$gatilho = New-ScheduledTaskTrigger -AtLogOn -User $usuario
$gatilho.Delay = 'PT15S'
$longa = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
    -ExecutionTimeLimit ([TimeSpan]::Zero) -MultipleInstances IgnoreNew
Register-ScheduledTask -TaskName 'memlimpa-bandeja' -Action (New-ScheduledTaskAction -Execute $destino) `
    -Trigger $gatilho -Principal $principal -Settings $longa -Force | Out-Null
"tarefa criada: memlimpa-bandeja (abre no login)"

Start-ScheduledTask -TaskName 'memlimpa-bandeja'
"pronto: o ícone está na bandeja."
Start-Sleep 3
