//! Canal privado entre a janela (sem privilégio) e a CLI elevada pelo UAC.
//!
//! O UAC (`ShellExecuteExW` + `runas`) não permite ligar stdin/stdout/stderr
//! do processo elevado. A janela cria três *named pipes* com nome aleatório
//! (128 bits), acesso só do usuário/administradores/SYSTEM e uma única
//! instância; a CLI recebe `--pipe <nonce>`, abre os três e os coloca como
//! sua entrada/saída/erro padrão. A janela confere que quem conectou é o
//! processo que ela abriu.

/// Qual dos três canais.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    /// Janela → CLI (linha `cancel`).
    In,
    /// CLI → janela (resultado JSON).
    Out,
    /// CLI → janela (progresso e erros).
    Err,
}

impl Which {
    pub const ALL: [Which; 3] = [Which::In, Which::Out, Which::Err];

    fn suffix(self) -> &'static str {
        match self {
            Which::In => "in",
            Which::Out => "out",
            Which::Err => "err",
        }
    }
}

pub fn pipe_name(nonce: &str, which: Which) -> String {
    format!(r"\\.\pipe\usbkeeper-{nonce}-{}", which.suffix())
}

pub fn nonce_hex(bytes: &[u8; 16]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Nonce aceito pela CLI: exatamente 32 dígitos hex minúsculos.
pub fn valid_nonce(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// DACL protegida: só o usuário (mesmo SID quando elevado), administradores
/// e SYSTEM; ninguém mais lê nem escreve.
pub fn sddl(user_sid: &str) -> String {
    format!("D:P(A;;GA;;;{user_sid})(A;;GA;;;BA)(A;;GA;;;SY)")
}

/// Um argumento citado segundo as regras de `CommandLineToArgvW` (as mesmas
/// que o Rust usa para ler `std::env::args` no Windows).
pub fn quote_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '\n', '"']) {
        return arg.to_string();
    }
    let mut out = String::from('"');
    let mut slashes = 0usize;
    for c in arg.chars() {
        match c {
            '\\' => slashes += 1,
            '"' => {
                out.extend(std::iter::repeat_n('\\', slashes * 2 + 1));
                out.push('"');
                slashes = 0;
            }
            _ => {
                out.extend(std::iter::repeat_n('\\', slashes));
                out.push(c);
                slashes = 0;
            }
        }
    }
    out.extend(std::iter::repeat_n('\\', slashes * 2));
    out.push('"');
    out
}

/// Parâmetros da CLI elevada: `--pipe <nonce>` e os argumentos da operação.
pub fn parameters(nonce: &str, args: &[String]) -> String {
    let mut all = vec!["--pipe".to_string(), nonce.to_string()];
    all.extend(args.iter().cloned());
    all.iter()
        .map(|a| quote_arg(a))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Separa `--pipe <nonce>` (logo depois do programa) do resto dos
/// argumentos. `Err` se `--pipe` vier com nonce inválido.
pub fn split_pipe_arg(
    mut args: Vec<std::ffi::OsString>,
) -> Result<(Option<String>, Vec<std::ffi::OsString>), String> {
    if args.get(1).and_then(|a| a.to_str()) != Some("--pipe") {
        return Ok((None, args));
    }
    let nonce = args
        .get(2)
        .and_then(|a| a.to_str())
        .filter(|n| valid_nonce(n))
        .map(String::from)
        .ok_or("--pipe exige um identificador de 32 dígitos hex")?;
    args.drain(1..3);
    Ok((Some(nonce), args))
}

#[cfg(windows)]
pub use imp::{Elevated, Process, attach, run_elevated};

#[cfg(windows)]
mod imp {
    use std::fs::File;
    use std::os::windows::io::FromRawHandle;
    use std::path::Path;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use windows::Win32::Foundation::{
        CloseHandle, ERROR_CANCELLED, ERROR_PIPE_CONNECTED, GENERIC_READ, GENERIC_WRITE, HANDLE,
        HLOCAL, INVALID_HANDLE_VALUE, LocalFree, WAIT_OBJECT_0,
    };
    use windows::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        SDDL_REVISION_1,
    };
    use windows::Win32::Security::Cryptography::{
        BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom,
    };
    use windows::Win32::Security::{
        GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
        TokenUser,
    };
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_SHARE_NONE,
        OPEN_EXISTING, PIPE_ACCESS_INBOUND, PIPE_ACCESS_OUTBOUND,
    };
    use windows::Win32::System::Console::{
        STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, SetStdHandle,
    };
    use windows::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId, PIPE_READMODE_BYTE,
        PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
    };
    use windows::Win32::System::Threading::{
        GetCurrentProcess, GetExitCodeProcess, GetProcessId, INFINITE, OpenProcessToken,
        TerminateProcess, WaitForSingleObject,
    };
    use windows::Win32::UI::Shell::{
        SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
    };
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
    use windows::core::{HSTRING, PCWSTR, PWSTR};

    use super::{Which, nonce_hex, parameters, pipe_name, sddl};

    /// Prazo para a CLI elevada conectar depois de o UAC ser aceito.
    const CONNECT_TIMEOUT: Duration = Duration::from_secs(60);

    /// Processo elevado com os três canais já conectados e conferidos.
    pub struct Elevated {
        pub stdin: File,
        pub stdout: File,
        pub stderr: File,
        pub process: Process,
    }

    /// O processo elevado (para esperar o fim).
    pub struct Process(Owned);

    impl Process {
        /// Espera a CLI terminar e devolve o código de saída.
        pub fn wait(self) -> Option<i32> {
            unsafe {
                WaitForSingleObject(self.0.0, INFINITE);
                let mut code = 0u32;
                GetExitCodeProcess(self.0.0, &mut code).ok()?;
                Some(code as i32)
            }
        }
    }

    /// HANDLE fechado ao sair do escopo.
    struct Owned(HANDLE);
    // SAFETY: um HANDLE de processo/pipe pode ser usado de qualquer thread.
    unsafe impl Send for Owned {}
    impl Drop for Owned {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    fn random_nonce() -> Result<String, String> {
        let mut bytes = [0u8; 16];
        unsafe { BCryptGenRandom(None, &mut bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }
            .ok()
            .map_err(|e| format!("sem gerador aleatório: {e}"))?;
        Ok(nonce_hex(&bytes))
    }

    fn user_sid() -> Result<String, String> {
        unsafe {
            let mut token = HANDLE::default();
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
                .map_err(|e| format!("token do processo: {e}"))?;
            let token = Owned(token);
            let mut len = 0u32;
            let _ = GetTokenInformation(token.0, TokenUser, None, 0, &mut len);
            let mut buf = vec![0u8; len as usize];
            GetTokenInformation(
                token.0,
                TokenUser,
                Some(buf.as_mut_ptr().cast()),
                len,
                &mut len,
            )
            .map_err(|e| format!("usuário do token: {e}"))?;
            let user = &*(buf.as_ptr() as *const TOKEN_USER);
            let mut s = PWSTR::null();
            ConvertSidToStringSidW(user.User.Sid, &mut s).map_err(|e| format!("SID: {e}"))?;
            let sid = s.to_string().map_err(|e| e.to_string());
            LocalFree(Some(HLOCAL(s.0.cast())));
            sid
        }
    }

    /// Descritor de segurança (liberado com `LocalFree` no Drop).
    struct Descriptor(PSECURITY_DESCRIPTOR);
    impl Drop for Descriptor {
        fn drop(&mut self) {
            unsafe {
                LocalFree(Some(HLOCAL(self.0.0)));
            }
        }
    }

    fn descriptor() -> Result<Descriptor, String> {
        let text = HSTRING::from(sddl(&user_sid()?));
        let mut sd = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                &text,
                SDDL_REVISION_1,
                &mut sd,
                None,
            )
        }
        .map_err(|e| format!("permissões do canal: {e}"))?;
        Ok(Descriptor(sd))
    }

    fn create_pipe(name: &str, which: Which, sd: &Descriptor) -> Result<Owned, String> {
        let access = match which {
            Which::In => PIPE_ACCESS_OUTBOUND,
            Which::Out | Which::Err => PIPE_ACCESS_INBOUND,
        };
        let sa = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd.0.0,
            bInheritHandle: false.into(),
        };
        let h = unsafe {
            CreateNamedPipeW(
                &HSTRING::from(name),
                access | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                64 * 1024,
                64 * 1024,
                0,
                Some(&sa),
            )
        };
        if h == INVALID_HANDLE_VALUE || h.is_invalid() {
            return Err(format!(
                "não foi possível criar o canal da operação: {}",
                windows::core::Error::from_win32()
            ));
        }
        Ok(Owned(h))
    }

    /// Abre o canal como cliente (usado só para destravar uma espera).
    fn poke(name: &str) {
        unsafe {
            if let Ok(h) = CreateFileW(
                &HSTRING::from(name),
                GENERIC_READ.0 | GENERIC_WRITE.0,
                FILE_SHARE_NONE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            ) {
                let _ = CloseHandle(h);
            }
        }
    }

    /// Abre a CLI com UAC e conecta os três canais. `Err("autorização
    /// negada")` se o usuário recusar o UAC.
    pub fn run_elevated(exe: &Path, args: &[String]) -> Result<Elevated, String> {
        let nonce = random_nonce()?;
        let sd = descriptor()?;
        let names = Which::ALL.map(|w| pipe_name(&nonce, w));
        let pipes = [
            create_pipe(&names[0], Which::In, &sd)?,
            create_pipe(&names[1], Which::Out, &sd)?,
            create_pipe(&names[2], Which::Err, &sd)?,
        ];

        let file = HSTRING::from(exe.as_os_str());
        let params = HSTRING::from(parameters(&nonce, args));
        let verb = HSTRING::from("runas");
        let mut info = SHELLEXECUTEINFOW {
            cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
            lpVerb: PCWSTR(verb.as_ptr()),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(params.as_ptr()),
            nShow: SW_HIDE.0,
            ..Default::default()
        };
        if let Err(e) = unsafe { ShellExecuteExW(&mut info) } {
            return Err(if e.code() == ERROR_CANCELLED.to_hresult() {
                "autorização negada".into()
            } else {
                format!("não foi possível iniciar {}: {e}", exe.display())
            });
        }
        if info.hProcess.is_invalid() {
            return Err("o Windows não devolveu o processo da operação".into());
        }
        let process = Owned(info.hProcess);
        let pid = unsafe { GetProcessId(process.0) };

        // ConnectNamedPipe bloqueia: espera numa thread e, se a CLI morrer
        // ou demorar, destrava abrindo o canal nós mesmos (o PID então não
        // confere e a operação falha).
        let raw = pipes.each_ref().map(|p| p.0.0 as usize);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for h in raw {
                let r = unsafe { ConnectNamedPipe(HANDLE(h as _), None) };
                let ok = match r {
                    Ok(()) => true,
                    Err(e) => e.code() == ERROR_PIPE_CONNECTED.to_hresult(),
                };
                if !ok {
                    break;
                }
            }
            let _ = tx.send(());
        });
        let started = Instant::now();
        let fail = loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break None,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            let exited = unsafe { WaitForSingleObject(process.0, 0) } == WAIT_OBJECT_0;
            if exited {
                break Some("a CLI terminou antes de se conectar à janela");
            }
            if started.elapsed() > CONNECT_TIMEOUT {
                unsafe {
                    let _ = TerminateProcess(process.0, 1);
                }
                break Some("a CLI não respondeu");
            }
        };
        if let Some(msg) = fail {
            names.iter().for_each(|n| poke(n));
            let _ = rx.recv_timeout(Duration::from_secs(5));
            return Err(msg.into());
        }
        for p in &pipes {
            let mut client = 0u32;
            let same =
                unsafe { GetNamedPipeClientProcessId(p.0, &mut client) }.is_ok() && client == pid;
            if !same {
                unsafe {
                    let _ = TerminateProcess(process.0, 1);
                }
                return Err("conexão inesperada no canal da operação".into());
            }
        }
        let [i, o, e] = pipes;
        let take = |p: Owned| {
            let h = p.0;
            std::mem::forget(p);
            unsafe { File::from_raw_handle(h.0) }
        };
        Ok(Elevated {
            stdin: take(i),
            stdout: take(o),
            stderr: take(e),
            process: Process(process),
        })
    }

    /// Lado da CLI: conecta aos canais e os torna a entrada/saída/erro padrão.
    pub fn attach(nonce: &str) -> Result<(), String> {
        let open = |which: Which, access: u32| unsafe {
            CreateFileW(
                &HSTRING::from(pipe_name(nonce, which)),
                access,
                FILE_SHARE_NONE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
            .map_err(|e| format!("canal {which:?}: {e}"))
        };
        let i = open(Which::In, GENERIC_READ.0)?;
        let o = open(Which::Out, GENERIC_WRITE.0)?;
        let e = open(Which::Err, GENERIC_WRITE.0)?;
        unsafe {
            SetStdHandle(STD_INPUT_HANDLE, i).map_err(|e| e.to_string())?;
            SetStdHandle(STD_OUTPUT_HANDLE, o).map_err(|e| e.to_string())?;
            SetStdHandle(STD_ERROR_HANDLE, e).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn names_and_nonce() {
        let n = nonce_hex(&[0xab; 16]);
        assert_eq!(n.len(), 32);
        assert!(valid_nonce(&n));
        assert_eq!(
            pipe_name(&n, Which::Err),
            format!(r"\\.\pipe\usbkeeper-{n}-err")
        );
        assert!(!valid_nonce("ABABABABABABABABABABABABABABABAB"));
        assert!(!valid_nonce("ab"));
        assert!(!valid_nonce(r"..\..\x0000000000000000000000000000"));
    }

    #[test]
    fn dacl_only_user_admins_system() {
        assert_eq!(
            sddl("S-1-5-21-1-2-3-1001"),
            "D:P(A;;GA;;;S-1-5-21-1-2-3-1001)(A;;GA;;;BA)(A;;GA;;;SY)"
        );
    }

    #[test]
    fn quoting_follows_commandline_to_argv() {
        assert_eq!(quote_arg("format"), "format");
        assert_eq!(quote_arg(r"\\.\PhysicalDrive1"), r"\\.\PhysicalDrive1");
        assert_eq!(quote_arg("MEU PEN"), "\"MEU PEN\"");
        assert_eq!(quote_arg(""), "\"\"");
        assert_eq!(quote_arg(r#"a"b"#), r#""a\"b""#);
        assert_eq!(
            quote_arg(r"C:\Program Files\x\"),
            r#""C:\Program Files\x\\""#
        );
        assert_eq!(quote_arg(r#"a\"b c"#), r#""a\\\"b c""#);
    }

    #[test]
    fn parameters_start_with_pipe() {
        let p = parameters(
            "0123456789abcdef0123456789abcdef",
            &["format".into(), "--label".into(), "MEU PEN".into()],
        );
        assert_eq!(
            p,
            "--pipe 0123456789abcdef0123456789abcdef format --label \"MEU PEN\""
        );
    }

    #[test]
    fn split_pipe() {
        let v = |a: &[&str]| a.iter().map(OsString::from).collect::<Vec<_>>();
        let (n, rest) = split_pipe_arg(v(&[
            "usbkeeper",
            "--pipe",
            "0123456789abcdef0123456789abcdef",
            "list",
        ]))
        .unwrap();
        assert_eq!(n.as_deref(), Some("0123456789abcdef0123456789abcdef"));
        assert_eq!(rest, v(&["usbkeeper", "list"]));
        let (n, rest) = split_pipe_arg(v(&["usbkeeper", "list"])).unwrap();
        assert!(n.is_none() && rest.len() == 2);
        assert!(split_pipe_arg(v(&["usbkeeper", "--pipe", "x"])).is_err());
        assert!(split_pipe_arg(v(&["usbkeeper", "--pipe"])).is_err());
        // --pipe fora da primeira posição não é tratado aqui (o clap recusa).
        let (n, _) = split_pipe_arg(v(&["usbkeeper", "list", "--pipe", "x"])).unwrap();
        assert!(n.is_none());
    }
}
