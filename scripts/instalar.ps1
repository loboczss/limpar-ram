# Instala o memlimpa para o usuário atual (pede permissão de administrador uma vez):
#   1. copia o memlimpa.exe para %ProgramFiles%\memlimpa e põe essa pasta no PATH
#   2. cria as tarefas agendadas que rodam elevadas sem a janela do UAC:
#        memlimpa          -> schtasks /run /tn memlimpa        (limpeza padrão)
#        memlimpa-tudo     -> schtasks /run /tn memlimpa-tudo   (limpeza completa)
#        memlimpa-bandeja  -> ícone na bandeja, abre no login
#   3. abre o ícone na bandeja
#
# Por que Program Files: as tarefas rodam como administrador. Se o .exe ficasse numa pasta
# que o usuário pode alterar, qualquer programa sem admin poderia trocá-lo e ganhar admin.
#
# Uso:  powershell -ExecutionPolicy Bypass -File scripts\instalar.ps1 [-Exe caminho\memlimpa.exe]
param([string]$Exe)
$ErrorActionPreference = 'Stop'

# Procura o .exe ao lado do script (zip da Release) ou na pasta de build (clone do repositório).
if (-not $Exe) {
    $Exe = @((Join-Path $PSScriptRoot 'memlimpa.exe'), (Join-Path $PSScriptRoot '..\target\release\memlimpa.exe')) |
        Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $Exe) { throw 'memlimpa.exe não encontrado. Baixe da página de Releases ou rode cargo build --release.' }
}

$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) {
    $origem = (Resolve-Path $Exe).Path
    Start-Process powershell -Verb RunAs -Wait -ArgumentList "-ExecutionPolicy Bypass -File `"$PSCommandPath`" -Exe `"$origem`""
    exit
}

$pasta = Join-Path $env:ProgramFiles 'memlimpa'
$destino = Join-Path $pasta 'memlimpa.exe'
New-Item -ItemType Directory -Force $pasta | Out-Null
Get-Process memlimpa -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500
Copy-Item $Exe $destino -Force
"copiado: $destino"

# Versões antigas instalavam em %USERPROFILE%\bin (pasta gravável pelo usuário): remove.
$antigo = Join-Path $env:USERPROFILE 'bin\memlimpa.exe'
if (Test-Path $antigo) { Remove-Item $antigo -Force; "removida a cópia antiga: $antigo" }

# Coloca a pasta no PATH do usuário, se ainda não estiver.
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
