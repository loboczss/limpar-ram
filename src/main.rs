#![windows_subsystem = "windows"]

mod bandeja;
mod config;
mod limpeza;

use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};

pub fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

const AJUDA: &str = "\
memlimpa - limpa a memória RAM do Windows

  memlimpa                 abre o ícone na bandeja
  memlimpa /limpar         limpa com as regiões configuradas e sai
  memlimpa /limpar tudo    inclui as listas standby e modificada (pode travar alguns segundos)
  memlimpa /limpar 0x3F    usa uma máscara de regiões específica
  memlimpa /?              esta ajuda

A limpeza precisa de administrador. Saída: 0 = ok, 1 = sem permissão, 2 = alguma região falhou.
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let primeiro = args.first().map(|s| s.to_lowercase());
    match primeiro.as_deref() {
        Some("/limpar" | "--limpar" | "/clean" | "-clean") => std::process::exit(cli(args.get(1))),
        Some("/?" | "-h" | "--help" | "/ajuda") => {
            console();
            print!("{AJUDA}");
        }
        _ => bandeja::rodar(
            args.iter().any(|a| a == "--reiniciar"),
            args.iter().any(|a| a == "--limpar-ao-abrir"),
        ),
    }
}

/// Programa de janela não tem console; pega emprestado o de quem chamou.
fn console() {
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

fn cli(opcao: Option<&String>) -> i32 {
    console();
    if !limpeza::eh_admin() {
        eprintln!("memlimpa: precisa rodar como administrador.");
        return 1;
    }
    let mascara = match opcao.map(|s| s.to_lowercase()) {
        None => config::Config::carregar().mascara,
        Some(s) if s == "tudo" || s == "all" => limpeza::TODAS,
        Some(s) => match u32::from_str_radix(s.trim_start_matches("0x"), 16) {
            Ok(n) => n & limpeza::TODAS,
            Err(_) => {
                eprintln!("memlimpa: opção desconhecida '{s}'. Use /? para ajuda.");
                return 1;
            }
        },
    };
    let antes = limpeza::memoria().carga;
    let r = limpeza::limpar(mascara);
    let m = limpeza::memoria();
    let mut texto = format!(
        "Liberado: {}  (RAM {}% -> {}%, {} livres de {})",
        limpeza::tamanho(r.liberado),
        antes,
        m.carga,
        limpeza::tamanho(m.livre),
        limpeza::tamanho(m.total)
    );
    for e in &r.erros {
        texto.push_str(&format!("\nfalhou: {e}"));
    }
    println!("{texto}");
    registrar(&texto);
    if r.erros.is_empty() { 0 } else { 2 }
}

/// Grava o último resultado em %LOCALAPPDATA%\memlimpa\ultima-limpeza.txt,
/// para quem chamou via tarefa agendada (sem console) poder ler.
fn registrar(texto: &str) {
    let Some(base) = std::env::var_os("LOCALAPPDATA") else { return };
    let agora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    gravar_na_pasta_do_usuario(
        &std::path::PathBuf::from(base).join("memlimpa"),
        "ultima-limpeza.txt",
        &format!("unix={agora}\n{texto}\n"),
    );
}

fn eh_atalho(p: &std::path::Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    std::fs::symlink_metadata(p).is_ok_and(|m| m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
}

/// Grava `arquivo` em `pasta` (dentro do perfil do usuário). Rodando como admin, uma
/// junction/symlink plantada ali redirecionaria a escrita para pastas do sistema; então
/// recusa se a pasta, a pasta de cima ou o arquivo forem pontos de reanálise.
pub fn gravar_na_pasta_do_usuario(pasta: &std::path::Path, arquivo: &str, conteudo: &str) {
    if pasta.parent().is_some_and(eh_atalho) || eh_atalho(pasta) {
        return;
    }
    if std::fs::create_dir_all(pasta).is_err() || eh_atalho(pasta) {
        return;
    }
    let destino = pasta.join(arquivo);
    if eh_atalho(&destino) {
        return;
    }
    let _ = std::fs::write(destino, conteudo);
}
