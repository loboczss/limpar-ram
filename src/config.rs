use std::{fs, path::PathBuf};

use crate::limpeza;

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

fn caminho() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("memlimpa").join("config.ini")
}

impl Config {
    pub fn carregar() -> Self {
        let mut c = Self::default();
        let Ok(texto) = fs::read_to_string(caminho()) else { return c };
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
        let p = caminho();
        let Some(pasta) = p.parent() else { return };
        crate::gravar_na_pasta_do_usuario(
            pasta,
            "config.ini",
            &format!(
                "mascara=0x{:02X}\nauto_porcentagem={}\nauto_minutos={}\navisos={}\n",
                self.mascara, self.auto_porcentagem, self.auto_minutos, self.avisos as u8
            ),
        );
    }
}
