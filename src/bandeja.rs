//! Ícone na bandeja com a % de RAM, menu e limpeza automática.
//! Win32 puro: uma janela invisível recebe os cliques do ícone e um timer.

use std::{
    cell::RefCell,
    ffi::c_void,
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
    sync::atomic::{AtomicU32, Ordering},
    time::{Duration, Instant},
};

use windows_sys::Win32::{
    Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, LRESULT, POINT, RECT, WAIT_ABANDONED, WAIT_OBJECT_0, WPARAM},
    Graphics::Gdi::*,
    System::{LibraryLoader::GetModuleHandleW, Threading::{CreateMutexW, WaitForSingleObject}},
    UI::{
        HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2},
        Shell::*,
        WindowsAndMessaging::*,
    },
};

use crate::{config::Config, limpeza, w};

const WM_ICONE: u32 = WM_APP + 1;
const WM_LIMPEZA_FEITA: u32 = WM_APP + 2;
const TIMER: usize = 1;
const INTERVALO_MS: u32 = 2000;
/// Depois de uma limpeza automática por %, espera isso antes de outra.
const ESPERA_AUTO: Duration = Duration::from_secs(60);

const ID_LIMPAR: usize = 100;
const ID_ADMIN: usize = 101;
const ID_AVISOS: usize = 102;
const ID_SAIR: usize = 103;
const ID_LIMPAR_TUDO: usize = 104;
const ID_PCT: usize = 200;
const ID_MIN: usize = 300;
const ID_REGIAO: usize = 400;

const OPCOES_PCT: [u32; 6] = [0, 70, 80, 85, 90, 95];
const OPCOES_MIN: [u32; 7] = [0, 5, 10, 15, 30, 60, 120];

#[derive(Clone, Copy, PartialEq)]
enum Origem {
    Manual,
    Auto,
    /// Manual incluindo as listas que podem travar.
    Tudo,
}

struct Estado {
    cfg: Config,
    icone: HICON,
    pct_no_icone: u32,
    limpando: bool,
    ultima_auto: Option<Instant>,
    inicio_intervalo: Instant,
}

thread_local! {
    static ESTADO: RefCell<Option<Estado>> = const { RefCell::new(None) };
}
static MSG_TASKBAR: AtomicU32 = AtomicU32::new(0);

fn com_estado<R>(f: impl FnOnce(&mut Estado) -> R) -> R {
    ESTADO.with(|e| f(e.borrow_mut().as_mut().expect("estado")))
}

pub fn rodar(reiniciando: bool, limpar_ao_abrir: bool) {
    unsafe {
        // Uma instância só. Ao reabrir como admin, espera a antiga fechar.
        let mutex = CreateMutexW(null(), 1, w("Local\\memlimpa-bandeja").as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            if !reiniciando {
                return;
            }
            let r = WaitForSingleObject(mutex, 10_000);
            if r != WAIT_OBJECT_0 && r != WAIT_ABANDONED {
                return;
            }
        }

        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

        let inst = GetModuleHandleW(null());
        let classe = w("memlimpa_janela");
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(janela),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: inst,
            hIcon: null_mut(),
            hCursor: null_mut(),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: classe.as_ptr(),
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(0, classe.as_ptr(), w("memlimpa").as_ptr(), 0, 0, 0, 0, 0, null_mut(), null_mut(), inst, null());
        if hwnd.is_null() {
            return;
        }

        // Se o Explorer reiniciar, ele avisa com essa mensagem e o ícone volta.
        let msg = RegisterWindowMessageW(w("TaskbarCreated").as_ptr());
        MSG_TASKBAR.store(msg, Ordering::Relaxed);
        // Rodando como admin, o Windows bloqueia mensagens vindas do Explorer; libera estas duas.
        ChangeWindowMessageFilterEx(hwnd, msg, MSGFLT_ALLOW, null_mut());
        ChangeWindowMessageFilterEx(hwnd, WM_ICONE, MSGFLT_ALLOW, null_mut());

        ESTADO.with(|e| {
            *e.borrow_mut() = Some(Estado {
                cfg: Config::carregar(),
                icone: null_mut(),
                pct_no_icone: u32::MAX,
                limpando: false,
                ultima_auto: None,
                inicio_intervalo: Instant::now(),
            })
        });

        atualizar_icone(hwnd, true);
        SetTimer(hwnd, TIMER, INTERVALO_MS, None);

        if limpar_ao_abrir {
            pedir_limpeza(hwnd, Origem::Manual);
        }

        let mut m: MSG = zeroed();
        while GetMessageW(&mut m, null_mut(), 0, 0) > 0 {
            TranslateMessage(&m);
            DispatchMessageW(&m);
        }
    }
}

unsafe extern "system" fn janela(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_TIMER => {
            tique(hwnd);
            0
        }
        WM_ICONE => {
            match lp as u32 {
                WM_RBUTTONUP | WM_LBUTTONUP => menu(hwnd),
                WM_LBUTTONDBLCLK => pedir_limpeza(hwnd, Origem::Manual),
                _ => {}
            }
            0
        }
        WM_LIMPEZA_FEITA => {
            let r = Box::from_raw(lp as *mut limpeza::Resultado);
            let origem = match wp {
                1 => Origem::Auto,
                2 => Origem::Tudo,
                _ => Origem::Manual,
            };
            limpeza_feita(hwnd, *r, origem);
            0
        }
        WM_DESTROY => {
            let n = base_icone(hwnd);
            Shell_NotifyIconW(NIM_DELETE, &n);
            PostQuitMessage(0);
            0
        }
        _ if msg != 0 && msg == MSG_TASKBAR.load(Ordering::Relaxed) => {
            atualizar_icone(hwnd, true);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

fn copiar(destino: &mut [u16], texto: &str) {
    let mut i = 0;
    for c in texto.encode_utf16() {
        if i + 1 >= destino.len() {
            break;
        }
        destino[i] = c;
        i += 1;
    }
    destino[i] = 0;
}

unsafe fn base_icone(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut n: NOTIFYICONDATAW = zeroed();
    n.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    n.hWnd = hwnd;
    n.uID = 1;
    n
}

/// Desenha um quadrado colorido com a % no meio.
unsafe fn desenhar_icone(pct: u32) -> HICON {
    let tam = GetSystemMetrics(SM_CXSMICON).max(16);
    let tela = GetDC(null_mut());
    let dc = CreateCompatibleDC(tela);

    let mut bmi: BITMAPINFO = zeroed();
    bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = tam;
    bmi.bmiHeader.biHeight = -tam;
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;
    let mut bits: *mut c_void = null_mut();
    let cor = CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
    let antigo = SelectObject(dc, cor);

    let rgb = |r: u32, g: u32, b: u32| r | (g << 8) | (b << 16);
    let fundo = match pct {
        0..=69 => rgb(0x1f, 0x7a, 0x3a),
        70..=84 => rgb(0xb4, 0x6a, 0x00),
        _ => rgb(0xc0, 0x26, 0x26),
    };
    let mut r = RECT { left: 0, top: 0, right: tam, bottom: tam };
    let pincel = CreateSolidBrush(fundo);
    FillRect(dc, &r, pincel);
    DeleteObject(pincel);

    let altura = if pct >= 100 { tam * 55 / 100 } else { tam * 80 / 100 };
    let fonte = CreateFontW(
        -altura, 0, 0, 0, FW_BOLD as i32, 0, 0, 0,
        DEFAULT_CHARSET as u32, OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32,
        ANTIALIASED_QUALITY as u32, (DEFAULT_PITCH | FF_DONTCARE) as u32,
        w("Segoe UI").as_ptr(),
    );
    let fonte_antiga = SelectObject(dc, fonte);
    SetBkMode(dc, TRANSPARENT as _);
    SetTextColor(dc, rgb(255, 255, 255));
    let mut texto: Vec<u16> = pct.min(100).to_string().encode_utf16().collect();
    DrawTextW(dc, texto.as_mut_ptr(), texto.len() as i32, &mut r, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
    GdiFlush();

    // O GDI não preenche o canal alfa; deixa tudo opaco.
    let px = std::slice::from_raw_parts_mut(bits as *mut u32, (tam * tam) as usize);
    for p in px {
        *p |= 0xFF00_0000;
    }

    SelectObject(dc, fonte_antiga);
    SelectObject(dc, antigo);
    let mascara = CreateBitmap(tam, tam, 1, 1, null());
    let info = ICONINFO { fIcon: 1, xHotspot: 0, yHotspot: 0, hbmMask: mascara, hbmColor: cor };
    let icone = CreateIconIndirect(&info);

    DeleteObject(fonte);
    DeleteObject(mascara);
    DeleteObject(cor);
    DeleteDC(dc);
    ReleaseDC(null_mut(), tela);
    icone
}

/// Atualiza ícone e dica. `adicionar` recria o ícone (início ou Explorer reiniciado).
unsafe fn atualizar_icone(hwnd: HWND, adicionar: bool) {
    let m = limpeza::memoria();
    let dica = format!(
        "memlimpa: RAM em {}%\n{} de {} em uso",
        m.carga,
        limpeza::tamanho(m.usada()),
        limpeza::tamanho(m.total)
    );

    let novo = com_estado(|e| {
        if adicionar || e.pct_no_icone != m.carga {
            e.pct_no_icone = m.carga;
            let velho = std::mem::replace(&mut e.icone, desenhar_icone(m.carga));
            Some((e.icone, velho))
        } else {
            None
        }
    });

    let mut n = base_icone(hwnd);
    n.uFlags = NIF_TIP | NIF_MESSAGE;
    n.uCallbackMessage = WM_ICONE;
    copiar(&mut n.szTip, &dica);
    if let Some((icone, _)) = novo {
        n.uFlags |= NIF_ICON;
        n.hIcon = icone;
    }
    if adicionar {
        Shell_NotifyIconW(NIM_ADD, &n);
    } else {
        Shell_NotifyIconW(NIM_MODIFY, &n);
    }
    if let Some((_, velho)) = novo {
        if !velho.is_null() {
            DestroyIcon(velho);
        }
    }
}

unsafe fn aviso(hwnd: HWND, titulo: &str, texto: &str) {
    let mut n = base_icone(hwnd);
    n.uFlags = NIF_INFO;
    n.dwInfoFlags = NIIF_INFO | NIIF_NOSOUND;
    copiar(&mut n.szInfoTitle, titulo);
    copiar(&mut n.szInfo, texto);
    Shell_NotifyIconW(NIM_MODIFY, &n);
}

unsafe fn tique(hwnd: HWND) {
    atualizar_icone(hwnd, false);

    let carga = limpeza::memoria().carga;
    let disparar = com_estado(|e| {
        if e.limpando || !limpeza::eh_admin() {
            return false;
        }
        let por_pct = e.cfg.auto_porcentagem > 0
            && carga >= e.cfg.auto_porcentagem
            && e.ultima_auto.map_or(true, |t| t.elapsed() >= ESPERA_AUTO);
        let por_tempo = e.cfg.auto_minutos > 0
            && e.inicio_intervalo.elapsed() >= Duration::from_secs(e.cfg.auto_minutos as u64 * 60);
        por_pct || por_tempo
    });
    if disparar {
        pedir_limpeza(hwnd, Origem::Auto);
    }
}

unsafe fn pedir_limpeza(hwnd: HWND, origem: Origem) {
    if !limpeza::eh_admin() {
        reabrir_como_admin(hwnd, true);
        return;
    }
    let mascara = com_estado(|e| {
        if e.limpando {
            return None;
        }
        e.limpando = true;
        // Igual ao original: a automática não mexe nas listas que podem travar o PC.
        Some(match origem {
            Origem::Auto => e.cfg.mascara & !limpeza::TRAVAM,
            Origem::Tudo => limpeza::TODAS,
            Origem::Manual => e.cfg.mascara,
        })
    });
    let Some(mascara) = mascara else { return };

    let janela = hwnd as usize;
    std::thread::spawn(move || {
        let r = Box::new(limpeza::limpar(mascara));
        let wp = match origem {
            Origem::Manual => 0,
            Origem::Auto => 1,
            Origem::Tudo => 2,
        };
        PostMessageW(janela as HWND, WM_LIMPEZA_FEITA, wp, Box::into_raw(r) as LPARAM);
    });
}

unsafe fn limpeza_feita(hwnd: HWND, r: limpeza::Resultado, origem: Origem) {
    let avisos = com_estado(|e| {
        e.limpando = false;
        e.inicio_intervalo = Instant::now();
        if origem == Origem::Auto {
            e.ultima_auto = Some(Instant::now());
        }
        e.cfg.avisos
    });
    atualizar_icone(hwnd, false);
    if avisos || !r.erros.is_empty() {
        let titulo = match origem {
            Origem::Auto => "Limpeza automática",
            Origem::Tudo => "Memória limpa (completa)",
            Origem::Manual => "Memória limpa",
        };
        let mut texto = format!("Liberado: {}", limpeza::tamanho(r.liberado));
        if !r.erros.is_empty() {
            texto.push_str(&format!("\nFalhou: {}", r.erros.join(", ")));
        }
        aviso(hwnd, titulo, &texto);
    }
}

unsafe fn reabrir_como_admin(hwnd: HWND, limpar: bool) {
    let Ok(exe) = std::env::current_exe() else { return };
    let exe = w(&exe.to_string_lossy());
    let args = w(if limpar { "--reiniciar --limpar-ao-abrir" } else { "--reiniciar" });
    let r = ShellExecuteW(hwnd, w("runas").as_ptr(), exe.as_ptr(), args.as_ptr(), null(), SW_SHOWNORMAL);
    if r as isize > 32 {
        // A nova instância (admin) assume o ícone.
        DestroyWindow(hwnd);
    }
}

unsafe fn item(menu: HMENU, id: usize, texto: &str, marcado: bool, ativo: bool) {
    let mut f = MF_STRING;
    if marcado {
        f |= MF_CHECKED;
    }
    if !ativo {
        f |= MF_GRAYED;
    }
    AppendMenuW(menu, f, id, w(texto).as_ptr());
}

unsafe fn separador(menu: HMENU) {
    AppendMenuW(menu, MF_SEPARATOR, 0, null());
}

unsafe fn menu(hwnd: HWND) {
    let admin = limpeza::eh_admin();
    let m = limpeza::memoria();
    let (mascara, pct, min, avisos, limpando) =
        com_estado(|e| (e.cfg.mascara, e.cfg.auto_porcentagem, e.cfg.auto_minutos, e.cfg.avisos, e.limpando));

    let raiz = CreatePopupMenu();
    item(raiz, 0, &format!("RAM em {}%: {} de {}", m.carga, limpeza::tamanho(m.usada()), limpeza::tamanho(m.total)), false, false);
    separador(raiz);
    item(raiz, ID_LIMPAR, if limpando { "Limpando..." } else { "Limpar agora" }, false, !limpando);
    item(raiz, ID_LIMPAR_TUDO, "Limpar tudo agora (inclui standby; pode travar uns segundos)", false, !limpando);
    SetMenuDefaultItem(raiz, ID_LIMPAR as u32, 0);
    if !admin {
        item(raiz, ID_ADMIN, "Reabrir como administrador (para a limpeza automática)", false, true);
    }
    separador(raiz);

    let sub_pct = CreatePopupMenu();
    for (i, &v) in OPCOES_PCT.iter().enumerate() {
        let t = if v == 0 { "Desligado".to_string() } else { format!("{v}%") };
        item(sub_pct, ID_PCT + i, &t, v == pct, true);
    }
    AppendMenuW(raiz, MF_POPUP, sub_pct as usize, w("Limpar quando a RAM passar de").as_ptr());

    let sub_min = CreatePopupMenu();
    for (i, &v) in OPCOES_MIN.iter().enumerate() {
        let t = if v == 0 { "Desligado".to_string() } else { format!("{v} minutos") };
        item(sub_min, ID_MIN + i, &t, v == min, true);
    }
    AppendMenuW(raiz, MF_POPUP, sub_min as usize, w("Limpar a cada").as_ptr());

    let sub_reg = CreatePopupMenu();
    for (i, &(bit, nome)) in limpeza::REGIOES.iter().enumerate() {
        item(sub_reg, ID_REGIAO + i, nome, mascara & bit != 0, true);
    }
    separador(sub_reg);
    item(sub_reg, 0, "* pode travar o PC por alguns segundos; a automática pula", false, false);
    AppendMenuW(raiz, MF_POPUP, sub_reg as usize, w("O que limpar").as_ptr());

    item(raiz, ID_AVISOS, "Mostrar aviso ao limpar", avisos, true);
    separador(raiz);
    item(raiz, ID_SAIR, "Sair", false, true);

    let mut p = POINT { x: 0, y: 0 };
    GetCursorPos(&mut p);
    SetForegroundWindow(hwnd);
    let escolha = TrackPopupMenu(raiz, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY, p.x, p.y, 0, hwnd, null()) as usize;
    PostMessageW(hwnd, WM_NULL, 0, 0);
    DestroyMenu(raiz); // destrói os submenus junto

    match escolha {
        0 => {}
        ID_LIMPAR => pedir_limpeza(hwnd, Origem::Manual),
        ID_LIMPAR_TUDO => pedir_limpeza(hwnd, Origem::Tudo),
        ID_ADMIN => reabrir_como_admin(hwnd, false),
        ID_SAIR => {
            DestroyWindow(hwnd);
        }
        ID_AVISOS => com_estado(|e| {
            e.cfg.avisos = !e.cfg.avisos;
            e.cfg.salvar();
        }),
        x if (ID_PCT..ID_PCT + OPCOES_PCT.len()).contains(&x) => com_estado(|e| {
            e.cfg.auto_porcentagem = OPCOES_PCT[x - ID_PCT];
            e.cfg.salvar();
        }),
        x if (ID_MIN..ID_MIN + OPCOES_MIN.len()).contains(&x) => com_estado(|e| {
            e.cfg.auto_minutos = OPCOES_MIN[x - ID_MIN];
            e.inicio_intervalo = Instant::now();
            e.cfg.salvar();
        }),
        x if (ID_REGIAO..ID_REGIAO + limpeza::REGIOES.len()).contains(&x) => com_estado(|e| {
            e.cfg.mascara ^= limpeza::REGIOES[x - ID_REGIAO].0;
            e.cfg.salvar();
        }),
        _ => {}
    }
}
