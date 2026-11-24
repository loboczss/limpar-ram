use std::{fs, path::PathBuf};

use crate::{arquivos, limpeza};

pub struct Config {
    pub mascara: u32,
    /// 0 = desligado
    pub auto_porcentagem: u32,
    /// 0 = desligado
    pub auto_minutos: u32,
    pub avisos: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { mascara: limpeza::PADRAO, auto_porcentagem: 0, auto_minutos: 0, avisos: true }
    }
}

const ARQUIVO: &str = "config.ini";

/// Usado só quando o programa roda SEM admin (aí gravar no perfil é seguro).
fn caminho_usuario() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|b| PathBuf::from(b).join("memlimpa").join(ARQUIVO))
}

fn ler_usuario() -> Option<String> {
    fs::read_to_string(caminho_usuario()?).ok()
}

impl Config {
    /// Elevado: lê de %ProgramData%\memlimpa e, se não houver, do %APPDATA% (migração).
    /// Sem admin: o contrário.
    pub fn carregar() -> Self {
        let texto = if limpeza::eh_admin() {
            arquivos::ler_sistema(ARQUIVO).or_else(ler_usuario)
        } else {
            ler_usuario().or_else(|| arquivos::ler_sistema(ARQUIVO))
        };
        let mut c = Self::default();
        let Some(texto) = texto else { return c };
        for linha in texto.lines() {
            let Some((chave, valor)) = linha.split_once('=') else { continue };
            let v = valor.trim();
            match chave.trim() {
                "mascara" => {
                    if let Ok(n) = u32::from_str_radix(v.trim_start_matches("0x"), 16) {
                        c.mascara = n & limpeza::TODAS;
                    }
                }
                "auto_porcentagem" => {
                    if let Ok(n) = v.parse::<u32>() {
                        c.auto_porcentagem = n.min(99);
                    }
                }
                "auto_minutos" => {
                    if let Ok(n) = v.parse::<u32>() {
                        c.auto_minutos = n.min(1440);
                    }
                }
                "avisos" => c.avisos = v == "1" || v.eq_ignore_ascii_case("true"),
                _ => {}
            }
        }
        c
    }

    pub fn salvar(&self) {
        let texto = format!(
            "mascara=0x{:02X}\nauto_porcentagem={}\nauto_minutos={}\navisos={}\n",
            self.mascara, self.auto_porcentagem, self.auto_minutos, self.avisos as u8
        );
        if limpeza::eh_admin() {
            arquivos::gravar_sistema(ARQUIVO, &texto);
        } else if let Some(p) = caminho_usuario() {
            if let Some(pasta) = p.parent() {
                let _ = fs::create_dir_all(pasta);
            }
            let _ = fs::write(p, texto);
        }
    }
}
