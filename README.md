<div align="center">

# 🧹 limpar-ram

**Um limpador de memória RAM para Windows: leve, rápido e direto ao ponto.**

Mostra a % de RAM ao lado do relógio, limpa com um duplo clique e limpa sozinho quando a memória passa do limite.

![Windows 10/11](https://img.shields.io/badge/Windows-10%20%7C%2011-0078D4?logo=windows&logoColor=white)
![Rust](https://img.shields.io/badge/feito%20em-Rust-B7410E?logo=rust&logoColor=white)
![Tamanho](https://img.shields.io/badge/.exe-~220%20KB-2ea043)
![Sem dependências](https://img.shields.io/badge/depend%C3%AAncias-zero-2ea043)
![Licença MIT](https://img.shields.io/badge/licen%C3%A7a-MIT-blue)
[![Baixar](https://img.shields.io/github/v/release/loboczss/limpar-ram?label=baixar&logo=github)](https://github.com/loboczss/limpar-ram/releases/latest)

<img src="assets/icones.svg" alt="O ícone da bandeja muda de cor conforme o uso da RAM" width="420">

</div>

---

## ✨ O que ele faz

| | |
|---|---|
| 📊 **Ícone com a % de RAM** | Fica na bandeja do sistema e muda de cor: verde, laranja, vermelho. Passe o mouse e veja quanto está em uso. |
| 🖱️ **Limpar com um clique** | Duplo clique no ícone limpa na hora. No menu tem *Limpar agora* e *Limpar tudo agora*. |
| 🤖 **Limpeza automática** | Quando a RAM passar de 70, 80, 85, 90 ou 95% e/ou a cada 5 a 120 minutos. |
| 🎯 **Você escolhe o que limpar** | 8 regiões da memória, uma por uma. As que podem travar o PC ficam desligadas por padrão. |
| ⌨️ **Linha de comando** | `memlimpa --limpar` para usar em scripts, atalhos e no Agendador de Tarefas. |
| 🚀 **Abre com o Windows** | Já como administrador, sem a janela do UAC toda vez. |
| 🪶 **Leve de verdade** | Um `.exe` de ~220 KB, sem instalar nada, usando ~10 MB de RAM. |

## 📥 Instalar

### Jeito fácil: baixar pronto

1. Baixe o **[`memlimpa-windows-x64.zip`](https://github.com/loboczss/limpar-ram/releases/latest/download/memlimpa-windows-x64.zip)** na [página de Releases](https://github.com/loboczss/limpar-ram/releases/latest).
2. Extraia numa pasta qualquer.
3. Clique com o botão direito em `instalar.ps1` → **Executar com o PowerShell**.

Só quer o programa, sem instalar? Baixe o [`memlimpa.exe`](https://github.com/loboczss/limpar-ram/releases/latest/download/memlimpa.exe) e abra com dois cliques.

> 🛡️ O Windows SmartScreen pode avisar "o Windows protegeu o computador", porque o .exe não tem assinatura digital paga. Clique em **Mais informações → Executar assim mesmo**. Os hashes SHA-256 estão em `SHA256SUMS.txt`, na Release.

### Compilando você mesmo

Você precisa do [Rust](https://rustup.rs) (com as ferramentas de build do Visual Studio, que o `rustup` oferece instalar).

```powershell
git clone https://github.com/loboczss/limpar-ram
cd limpar-ram
cargo build --release
powershell -ExecutionPolicy Bypass -File scripts\instalar.ps1
```

O instalador pede permissão de administrador **uma vez** e:

1. copia o `memlimpa.exe` para `C:\Program Files\memlimpa` e coloca essa pasta no PATH (é uma pasta que só administrador altera: como as tarefas rodam como admin, nenhum outro programa consegue trocar o .exe por outro);
2. cria três tarefas agendadas que rodam como administrador sem perguntar de novo;
3. abre o ícone na bandeja, que passa a abrir sozinho a cada login.

> 💡 Se o ícone não aparecer, ele está escondido na setinha `^` da barra de tarefas. Arraste para fora.

Para remover tudo: `powershell -ExecutionPolicy Bypass -File scripts\desinstalar.ps1`

## 🖱️ Usando

**Na bandeja:** clique no ícone para abrir o menu.

```
RAM em 74%: 23,5 GB de 31,8 GB
─────────────────────────────
Limpar agora                  ← também com duplo clique
Limpar tudo agora (inclui standby; pode travar uns segundos)
─────────────────────────────
Limpar quando a RAM passar de  ▸  Desligado · 70% · 80% · 85% · 90% · 95%
Limpar a cada                  ▸  Desligado · 5 · 10 · 15 · 30 · 60 · 120 minutos
O que limpar                   ▸  as 8 regiões
✓ Mostrar aviso ao limpar
─────────────────────────────
Sair
```

Ao terminar, aparece um aviso com quanto foi liberado.

**No terminal** (precisa ser como administrador):

```powershell
memlimpa --limpar          # limpa as regiões configuradas
memlimpa --limpar tudo     # inclui standby e lista modificada
memlimpa --limpar 0x3F     # máscara de regiões específica
memlimpa /?                # ajuda
```

```
Liberado: 6,3 GB  (RAM 81% -> 62%, 12,0 GB livres de 31,8 GB)
```

Códigos de saída: `0` ok · `1` sem permissão · `2` alguma região falhou.

**Sem terminal de administrador**, depois de rodar o instalador:

```powershell
schtasks /run /tn memlimpa         # limpeza padrão
schtasks /run /tn memlimpa-tudo    # limpeza completa
```

O resultado da última limpeza fica em `C:\ProgramData\memlimpa\ultima-limpeza.txt`.

## 🧠 O que é limpo

| Região | Padrão | O que faz |
|---|:---:|---|
| Memória dos programas (working set) | ✅ | Tira da RAM as páginas que os programas não estão usando agora |
| Cache de arquivos do sistema | ✅ | Esvazia o cache de leitura de arquivos |
| Gravar cache pendente nos discos | ✅ | Grava nos discos o que estava esperando para ser escrito |
| Lista standby de baixa prioridade | ✅ | Solta dados guardados "por via das dúvidas" que valem pouco |
| Cache do registro | ✅ | Grava o registro do Windows e solta o cache dele |
| Combinar páginas iguais | ✅ | Junta páginas de memória idênticas numa só |
| Lista de páginas modificadas ⚠️ | ⬜ | Grava no disco as páginas alteradas e solta elas |
| Lista standby ⚠️ | ⬜ | Solta todo o cache de dados guardados para reuso |

⚠️ Essas duas liberam mais, mas o PC pode **engasgar por alguns segundos** e os programas recarregam do disco depois. Por isso ficam fora do padrão, e **a limpeza automática sempre pula as duas**. Boa hora para usá-las: antes de abrir um jogo.

## 🔧 Como funciona por dentro

Tudo passa por uma única função interna do Windows, `NtSetSystemInformation` (do `ntdll.dll`), a mesma que o [Mem Reduct](https://github.com/henrypp/memreduct) usa. Conferimos isso no binário oficial do Mem Reduct 3.5.2 com engenharia reversa (Ghidra), e depois escrevemos um código próprio do zero:

| Região | Chamada |
|---|---|
| Working set | classe `0x50`, comando `2` (`MemoryEmptyWorkingSets`) |
| Cache de arquivos | classe `0x51` com mínimo e máximo = `-1` |
| Cache pendente dos discos | `FlushFileBuffers` em cada volume |
| Lista modificada | classe `0x50`, comando `3` (`MemoryFlushModifiedList`) |
| Standby | classe `0x50`, comando `4` (`MemoryPurgeStandbyList`) |
| Standby de baixa prioridade | classe `0x50`, comando `5` (`MemoryPurgeLowPriorityStandbyList`) |
| Cache do registro | classe `0x9B` (`SystemRegistryReconciliationInformation`) |
| Combinar páginas | classe `0x82` (`SystemCombinePhysicalMemoryInformation`) |

Antes de limpar, o programa liga os privilégios `SeProfileSingleProcessPrivilege` e `SeIncreaseQuotaPrivilege`, que o kernel exige.

**Estrutura do código:**

```
src/
├── main.rs      # linha de comando e entrada
├── limpeza.rs   # as chamadas ao kernel (o coração)
├── bandeja.rs   # ícone, menu, limpeza automática (Win32 puro)
├── config.rs    # leitura e gravação do config.ini
└── arquivos.rs  # gravação segura em C:\ProgramData\memlimpa
scripts/
├── instalar.ps1
└── desinstalar.ps1
```

Sem framework de interface: o ícone é desenhado na hora com GDI e o menu é o nativo do Windows. A única dependência é o [`windows-sys`](https://crates.io/crates/windows-sys), que só declara as funções do sistema.

## ⚙️ Configuração

Rodando como administrador (o normal, depois do instalador), fica em `C:\ProgramData\memlimpa\config.ini`. Sem admin, em `%APPDATA%\memlimpa\config.ini`. O menu já mexe em tudo, mas dá para editar na mão (com o ícone fechado; a pasta do ProgramData só aceita edição como admin):

```ini
mascara=0xE7          ; regiões (bits: 01 working set, 02 cache, 04 standby baixa,
                      ;  08 standby, 10 modificada, 20 combinar, 40 registro, 80 discos)
auto_porcentagem=85   ; 0 = desligado
auto_minutos=0        ; 0 = desligado
avisos=1
```

## ❓ Perguntas comuns

<details>
<summary><b>Limpar a RAM deixa o PC mais rápido?</b></summary>

Depende. O Windows já usa a RAM livre como cache, e isso é bom. Limpar ajuda quando a memória está quase cheia e o PC começa a usar o disco (fica lento, trava). Também ajuda antes de abrir algo pesado, como um jogo. Com a RAM folgada, não muda nada.
</details>

<details>
<summary><b>"Liberado: 0 MB". Não funcionou?</b></summary>

Funcionou: não tinha nada para soltar. Acontece quando a RAM já está folgada.
</details>

<details>
<summary><b>Por que precisa de administrador?</b></summary>

As listas de memória e o cache são do sistema inteiro, não de um programa só. O Windows só deixa mexer nelas com privilégios de administrador. O instalador cria tarefas agendadas para você não precisar confirmar toda vez.
</details>

<details>
<summary><b>"Memory Compression" continua alto depois de limpar</b></summary>

É normal e não é problema. Ele encolhe sozinho quando a pressão na memória cai.
</details>

<details>
<summary><b>No Git Bash, <code>/limpar</code> abre o ícone em vez de limpar</b></summary>

O Git Bash transforma `/limpar` num caminho de arquivo. Use `--limpar`.
</details>

## 🙏 Créditos

Inspirado no [Mem Reduct](https://github.com/henrypp/memreduct), do Henry++. Este projeto não usa código dele: só as mesmas funções públicas do Windows.

Licença [MIT](LICENSE).
