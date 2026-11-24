//! O núcleo: as mesmas chamadas ao kernel que o Mem Reduct 3.5.2 faz
//! (confirmado no binário oficial com REA/Ghidra: NtSetSystemInformation
//! com as classes 0x50, 0x51, 0x82 e 0x9B).

use std::{ffi::c_void, mem::size_of, ptr::null_mut};

use windows_sys::Win32::{
    Foundation::{CloseHandle, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE, LUID},
    Security::{
        AdjustTokenPrivileges, GetTokenInformation, LookupPrivilegeValueW, TokenElevation,
        LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_ELEVATION,
        TOKEN_PRIVILEGES, TOKEN_QUERY,
    },
    Storage::FileSystem::{
        CreateFileW, FindFirstVolumeW, FindNextVolumeW, FindVolumeClose, FlushFileBuffers,
        FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    },
    System::{
        SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
        Threading::{GetCurrentProcess, OpenProcessToken},
    },
};

use crate::w;

#[link(name = "ntdll", kind = "raw-dylib")]
extern "system" {
    fn NtSetSystemInformation(classe: u32, info: *mut c_void, tamanho: u32) -> i32;
}

// Classes de informação do sistema (SYSTEM_INFORMATION_CLASS)
const SYSTEM_MEMORY_LIST_INFORMATION: u32 = 0x50;
const SYSTEM_FILE_CACHE_INFORMATION_EX: u32 = 0x51;
const SYSTEM_COMBINE_PHYSICAL_MEMORY_INFORMATION: u32 = 0x82;
const SYSTEM_REGISTRY_RECONCILIATION_INFORMATION: u32 = 0x9B;

// Comandos de SystemMemoryListInformation (SYSTEM_MEMORY_LIST_COMMAND)
const MEMORY_EMPTY_WORKING_SETS: u32 = 2;
const MEMORY_FLUSH_MODIFIED_LIST: u32 = 3;
const MEMORY_PURGE_STANDBY_LIST: u32 = 4;
const MEMORY_PURGE_LOW_PRIORITY_STANDBY_LIST: u32 = 5;

// Regiões (mesmos bits do original, para a máscara ser compatível)
pub const WORKING_SET: u32 = 0x01;
pub const CACHE_ARQUIVOS: u32 = 0x02;
pub const STANDBY_BAIXA: u32 = 0x04;
pub const STANDBY: u32 = 0x08;
pub const LISTA_MODIFICADA: u32 = 0x10;
pub const COMBINAR: u32 = 0x20;
pub const REGISTRO: u32 = 0x40;
pub const CACHE_DISCOS: u32 = 0x80;

pub const TODAS: u32 = 0xFF;
/// O padrão deixa de fora as duas que podem travar o PC por alguns segundos.
pub const PADRAO: u32 = TODAS & !TRAVAM;
pub const TRAVAM: u32 = STANDBY | LISTA_MODIFICADA;

pub const REGIOES: [(u32, &str); 8] = [
    (WORKING_SET, "Memória dos programas (working set)"),
    (CACHE_ARQUIVOS, "Cache de arquivos do sistema"),
    (CACHE_DISCOS, "Gravar cache pendente nos discos"),
    (LISTA_MODIFICADA, "Lista de páginas modificadas *"),
    (STANDBY, "Lista standby *"),
    (STANDBY_BAIXA, "Lista standby de baixa prioridade"),
    (REGISTRO, "Cache do registro"),
    (COMBINAR, "Combinar páginas iguais"),
];

#[repr(C)]
#[derive(Default)]
struct SystemFileCacheInformation {
    current_size: usize,
    peak_size: usize,
    page_fault_count: u32,
    minimum_working_set: usize,
    maximum_working_set: usize,
    current_size_including_transition_in_pages: usize,
    peak_size_including_transition_in_pages: usize,
    transition_repurpose_count: u32,
    flags: u32,
}

#[repr(C)]
struct MemoryCombineInformationEx {
    handle: HANDLE,
    pages_combined: usize,
    flags: u32,
}

pub struct Memoria {
    pub carga: u32,
    pub total: u64,
    pub livre: u64,
}

impl Memoria {
    pub fn usada(&self) -> u64 {
        self.total.saturating_sub(self.livre)
    }
}

pub fn memoria() -> Memoria {
    unsafe {
        let mut m: MEMORYSTATUSEX = std::mem::zeroed();
        m.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
        GlobalMemoryStatusEx(&mut m);
        Memoria { carga: m.dwMemoryLoad, total: m.ullTotalPhys, livre: m.ullAvailPhys }
    }
}

pub fn eh_admin() -> bool {
    unsafe {
        let mut token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elev = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut tam = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elev as *mut _ as *mut c_void,
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut tam,
        );
        CloseHandle(token);
        ok != 0 && elev.TokenIsElevated != 0
    }
}

/// Liga os privilégios que o kernel exige para mexer nas listas e no cache.
fn ativar_privilegios() {
    unsafe {
        let mut token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) == 0 {
            return;
        }
        for nome in ["SeProfileSingleProcessPrivilege", "SeIncreaseQuotaPrivilege"] {
            let mut luid = LUID { LowPart: 0, HighPart: 0 };
            if LookupPrivilegeValueW(std::ptr::null(), w(nome).as_ptr(), &mut luid) == 0 {
                continue;
            }
            let tp = TOKEN_PRIVILEGES {
                PrivilegeCount: 1,
                Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }],
            };
            AdjustTokenPrivileges(token, 0, &tp, 0, null_mut(), null_mut());
        }
        CloseHandle(token);
    }
}

fn nt(classe: u32, info: *mut c_void, tamanho: usize) -> Result<(), i32> {
    let status = unsafe { NtSetSystemInformation(classe, info, tamanho as u32) };
    if status >= 0 { Ok(()) } else { Err(status) }
}

fn lista(comando: u32) -> Result<(), i32> {
    let mut c = comando;
    nt(SYSTEM_MEMORY_LIST_INFORMATION, &mut c as *mut _ as *mut c_void, size_of::<u32>())
}

/// Grava nos discos tudo o que está no cache esperando para ser escrito.
fn gravar_cache_dos_discos() {
    unsafe {
        let mut nome = [0u16; 260];
        let busca = FindFirstVolumeW(nome.as_mut_ptr(), nome.len() as u32);
        if busca == INVALID_HANDLE_VALUE {
            return;
        }
        loop {
            // "\\?\Volume{...}\" -> sem a barra final abre o volume, não a raiz
            let fim = nome.iter().position(|&c| c == 0).unwrap_or(nome.len());
            let mut caminho: Vec<u16> = nome[..fim].to_vec();
            if caminho.last() == Some(&(b'\\' as u16)) {
                caminho.pop();
            }
            caminho.push(0);
            let vol = CreateFileW(
                caminho.as_ptr(),
                GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                null_mut(),
            );
            if vol != INVALID_HANDLE_VALUE {
                FlushFileBuffers(vol);
                CloseHandle(vol);
            }
            if FindNextVolumeW(busca, nome.as_mut_ptr(), nome.len() as u32) == 0 {
                break;
            }
        }
        FindVolumeClose(busca);
    }
}

pub struct Resultado {
    pub liberado: u64,
    pub erros: Vec<String>,
}

/// Limpa as regiões marcadas em `mascara`. Precisa rodar como administrador.
pub fn limpar(mascara: u32) -> Resultado {
    ativar_privilegios();
    let antes = memoria().livre;
    let mut erros = Vec::new();
    let mut anota = |nome: &str, r: Result<(), i32>| {
        if let Err(s) = r {
            erros.push(format!("{nome}: 0x{:08X}", s as u32));
        }
    };

    if mascara & WORKING_SET != 0 {
        anota("working set", lista(MEMORY_EMPTY_WORKING_SETS));
    }
    if mascara & CACHE_ARQUIVOS != 0 {
        // mínimo e máximo = -1 manda o kernel esvaziar o cache de arquivos
        let mut sfci = SystemFileCacheInformation {
            minimum_working_set: usize::MAX,
            maximum_working_set: usize::MAX,
            ..Default::default()
        };
        anota(
            "cache de arquivos",
            nt(SYSTEM_FILE_CACHE_INFORMATION_EX, &mut sfci as *mut _ as *mut c_void, size_of::<SystemFileCacheInformation>()),
        );
    }
    if mascara & CACHE_DISCOS != 0 {
        gravar_cache_dos_discos();
    }
    if mascara & LISTA_MODIFICADA != 0 {
        anota("lista modificada", lista(MEMORY_FLUSH_MODIFIED_LIST));
    }
    if mascara & STANDBY != 0 {
        anota("standby", lista(MEMORY_PURGE_STANDBY_LIST));
    }
    if mascara & STANDBY_BAIXA != 0 {
        anota("standby baixa prioridade", lista(MEMORY_PURGE_LOW_PRIORITY_STANDBY_LIST));
    }
    if mascara & REGISTRO != 0 {
        anota("cache do registro", nt(SYSTEM_REGISTRY_RECONCILIATION_INFORMATION, null_mut(), 0));
    }
    if mascara & COMBINAR != 0 {
        let mut info = MemoryCombineInformationEx { handle: null_mut(), pages_combined: 0, flags: 0 };
        anota(
            "combinar páginas",
            nt(SYSTEM_COMBINE_PHYSICAL_MEMORY_INFORMATION, &mut info as *mut _ as *mut c_void, size_of::<MemoryCombineInformationEx>()),
        );
    }

    let depois = memoria().livre;
    Resultado { liberado: depois.saturating_sub(antes), erros }
}

/// "1,4 GB" / "350 MB"
pub fn tamanho(bytes: u64) -> String {
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB).replace('.', ",")
    } else {
        format!("{} MB", bytes / (1024 * 1024))
    }
}
