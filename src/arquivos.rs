//! Onde o processo elevado grava (configuração e resultado da última limpeza).
//!
//! Gravar como administrador numa pasta que o usuário controla é perigoso: uma
//! junction ou symlink plantada ali desviaria a escrita para pastas do sistema, e
//! até as variáveis de ambiente (`%LOCALAPPDATA%`) são alteráveis pelo usuário.
//! Por isso o processo elevado só grava em `%ProgramData%\memlimpa`:
//!   - o caminho vem do registro da máquina (SHGetKnownFolderPath), não do ambiente;
//!   - a pasta é criada com ACL restrita (SYSTEM e Administradores gravam, usuários só leem);
//!   - a checagem é feita no arquivo JÁ ABERTO (sem seguir reparse point, um link só,
//!     caminho final igual ao esperado), então trocar a pasta no meio não adianta.

use std::{
    mem::{size_of, zeroed},
    path::{Path, PathBuf},
    ptr::{null, null_mut},
};

use windows_sys::Win32::{
    Foundation::{
        CloseHandle, GetLastError, LocalFree, ERROR_ALREADY_EXISTS, GENERIC_READ, GENERIC_WRITE, HANDLE,
        INVALID_HANDLE_VALUE,
    },
    Security::{
        Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1},
        PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
    },
    Storage::FileSystem::{
        CreateDirectoryW, CreateFileW, FileDispositionInfo, GetFileInformationByHandle, GetFinalPathNameByHandleW,
        SetEndOfFile, SetFileInformationByHandle, SetFilePointerEx, WriteFile, BY_HANDLE_FILE_INFORMATION, DELETE,
        FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_REPARSE_POINT, FILE_BEGIN, FILE_DISPOSITION_INFO,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_NAME_NORMALIZED, FILE_SHARE_READ, OPEN_ALWAYS, VOLUME_NAME_DOS,
    },
    System::Com::CoTaskMemFree,
    UI::Shell::{FOLDERID_ProgramData, SHGetKnownFolderPath, KF_FLAG_DEFAULT},
};

use crate::w;

/// `C:\ProgramData\memlimpa`, lido do registro da máquina.
pub fn pasta_sistema() -> Option<PathBuf> {
    unsafe {
        let mut p = null_mut();
        if SHGetKnownFolderPath(&FOLDERID_ProgramData, KF_FLAG_DEFAULT as _, null_mut(), &mut p) != 0 || p.is_null() {
            return None;
        }
        let mut n = 0;
        while *p.add(n) != 0 {
            n += 1;
        }
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
        CoTaskMemFree(p as _);
        Some(PathBuf::from(s).join("memlimpa"))
    }
}

pub fn ler_sistema(nome: &str) -> Option<String> {
    std::fs::read_to_string(pasta_sistema()?.join(nome)).ok()
}

/// Cria a pasta com ACL restrita, se ainda não existir. Se já existir, a proteção
/// fica por conta da checagem do arquivo aberto (e o instalador corrige a ACL).
fn garantir_pasta(pasta: &Path) {
    unsafe {
        // P = DACL protegida (não herda). SYSTEM e Administradores: tudo. Usuários: ler e executar.
        let sddl = w("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;0x1200a9;;;BU)");
        let mut sd: PSECURITY_DESCRIPTOR = null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), SDDL_REVISION_1, &mut sd, null_mut()) == 0 {
            return;
        }
        let sa = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd,
            bInheritHandle: 0,
        };
        CreateDirectoryW(w(&pasta.to_string_lossy()).as_ptr(), &sa);
        LocalFree(sd as _);
    }
}

unsafe fn arquivo_confiavel(h: HANDLE, esperado: &Path) -> bool {
    let mut info: BY_HANDLE_FILE_INFORMATION = zeroed();
    if GetFileInformationByHandle(h, &mut info) == 0 {
        return false;
    }
    // Não é symlink/junction e não é hardlink para outro arquivo.
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 || info.nNumberOfLinks != 1 {
        return false;
    }
    let mut buf = [0u16; 1024];
    let n = GetFinalPathNameByHandleW(h, buf.as_mut_ptr(), buf.len() as u32, FILE_NAME_NORMALIZED | VOLUME_NAME_DOS) as usize;
    if n == 0 || n >= buf.len() {
        return false;
    }
    let real = String::from_utf16_lossy(&buf[..n]);
    let real = real.strip_prefix(r"\\?\").unwrap_or(&real);
    real.eq_ignore_ascii_case(&esperado.to_string_lossy())
}

/// Grava `conteudo` em `%ProgramData%\memlimpa\<nome>`. Devolve false se não deu
/// (sem permissão, ou a pasta/arquivo apontam para outro lugar).
pub fn gravar_sistema(nome: &str, conteudo: &str) -> bool {
    let Some(pasta) = pasta_sistema() else { return false };
    garantir_pasta(&pasta);
    let caminho = pasta.join(nome);
    unsafe {
        let h = CreateFileW(
            w(&caminho.to_string_lossy()).as_ptr(),
            GENERIC_READ | GENERIC_WRITE | DELETE,
            FILE_SHARE_READ,
            null(),
            OPEN_ALWAYS,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        );
        if h == INVALID_HANDLE_VALUE {
            return false;
        }
        let criado_agora = GetLastError() != ERROR_ALREADY_EXISTS;

        if !arquivo_confiavel(h, &caminho) {
            // Caiu em outro lugar: não escreve nada e desfaz o arquivo vazio que criou.
            if criado_agora {
                let apagar = FILE_DISPOSITION_INFO { DeleteFile: 1 };
                SetFileInformationByHandle(h, FileDispositionInfo, &apagar as *const _ as _, size_of::<FILE_DISPOSITION_INFO>() as u32);
            }
            CloseHandle(h);
            return false;
        }

        SetFilePointerEx(h, 0, null_mut(), FILE_BEGIN);
        SetEndOfFile(h);
        let bytes = conteudo.as_bytes();
        let mut escrito = 0u32;
        let ok = WriteFile(h, bytes.as_ptr(), bytes.len() as u32, &mut escrito, null_mut()) != 0;
        CloseHandle(h);
        ok && escrito as usize == bytes.len()
    }
}
