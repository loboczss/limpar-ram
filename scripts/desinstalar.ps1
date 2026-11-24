# Remove o memlimpa: configuração, tarefas agendadas e executável. Pede admin uma vez.
#
# As pastas do perfil do usuário são apagadas ANTES de pedir admin: apagar recursivamente
# como administrador numa pasta que o usuário controla poderia seguir uma junction plantada
# ali e apagar arquivos do sistema.
param([switch]$Elevado)

if (-not $Elevado) {
    foreach ($p in (Join-Path $env:APPDATA 'memlimpa'), (Join-Path $env:LOCALAPPDATA 'memlimpa')) {
        if (Test-Path $p) { Remove-Item $p -Recurse -Force -ErrorAction SilentlyContinue; "apagado: $p" }
    }
    Start-Process powershell -Verb RunAs -Wait -ArgumentList "-ExecutionPolicy Bypass -File `"$PSCommandPath`" -Elevado"
    exit
}

foreach ($t in 'memlimpa', 'memlimpa-tudo', 'memlimpa-bandeja') {
    Unregister-ScheduledTask -TaskName $t -Confirm:$false -ErrorAction SilentlyContinue
}
Get-Process memlimpa -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500

# Só pastas que apenas administradores podem alterar.
Remove-Item (Join-Path $env:ProgramFiles 'memlimpa') -Recurse -Force -ErrorAction SilentlyContinue

# %ProgramData%\memlimpa: só apaga se for uma pasta de verdade com dono admin/SYSTEM
# (como o instalador deixa). Se for junction, remove só o link, sem entrar nele.
$dados = Join-Path $env:ProgramData 'memlimpa'
if (Test-Path -LiteralPath $dados) {
    $item = Get-Item -LiteralPath $dados -Force
    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        [IO.Directory]::Delete($dados)
    } elseif ($item.GetAccessControl().GetOwner([Security.Principal.SecurityIdentifier]).Value -in 'S-1-5-18', 'S-1-5-32-544') {
        # Dono admin/SYSTEM: só admin cria coisas lá dentro, então o conteúdo é confiável.
        Remove-Item -LiteralPath $dados -Recurse -Force -ErrorAction SilentlyContinue
    } else {
        "não apaguei ${dados}: o dono não é administrador. Confira e apague na mão."
    }
}

$pasta = Join-Path $env:ProgramFiles 'memlimpa'
$path = [Environment]::GetEnvironmentVariable('Path', 'User')
$novo = (($path -split ';') | Where-Object { $_ -and $_ -ne $pasta }) -join ';'
if ($novo -ne $path) { [Environment]::SetEnvironmentVariable('Path', $novo, 'User') }
"memlimpa removido."
Start-Sleep 2
