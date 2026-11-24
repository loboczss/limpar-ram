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
    let pasta = std::path::PathBuf::from(base).join("memlimpa");
    let _ = std::fs::create_dir_all(&pasta);
    let agora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let _ = std::fs::write(pasta.join("ultima-limpeza.txt"), format!("unix={agora}\n{texto}\n"));
}
