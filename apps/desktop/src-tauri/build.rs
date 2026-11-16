fn main() {
    // O binário de TESTE precisa do Common-Controls v6, e ninguém dá isso a ele.
    //
    // A árvore importa `TaskDialogIndirect` (diálogos nativos, via tauri-plugin-dialog). Esse
    // símbolo NÃO existe no comctl32 5.82, que é o que `C:\Windows\System32\comctl32.dll`
    // entrega por padrão: quem tem `TaskDialogIndirect` é a versão 6, e ela só é carregada
    // quando o executável declara a dependência no manifesto. O binário do app ganha manifesto
    // do `tauri_build::build()`; o harness que o `cargo test` linka não ganha de ninguém.
    //
    // Sem isto o processo morre ANTES do primeiro teste, com
    // `exit code: 0xc0000139, STATUS_ENTRYPOINT_NOT_FOUND` e zero saída — o que não se parece em
    // nada com o que é. A pista custou caro: o sintoma foi lido como disputa pelo diretório de
    // build compartilhado (`build-dir` do cargo global), e só um `build-dir` isolado, limpo,
    // falhando igual, mostrou que era da árvore. Quem nomeou o culpado foi a tabela de importação
    // do PE, resolvida símbolo a símbolo: 432 conferidos, 1 ausente.
    //
    // `rustc-link-arg`, e não `-tests`: este último só alcança alvos de `tests/`, e a suíte que
    // importa aqui é a do `--lib`, que é outro alvo (`cargo` recusa o `-tests` com "does not have
    // a test target"). Declarar a dependência também no binário do app é inofensivo — ele já
    // depende do mesmo assembly, e entradas repetidas de manifesto são fundidas.
    #[cfg(target_os = "windows")]
    println!(
        "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' \
         name='Microsoft.Windows.Common-Controls' version='6.0.0.0' \
         processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
    );

    // Build Steam no Linux: `libsteam_api.so` fica ao lado do executável no depot e o loader tem de
    // procurá-la ali, sem depender do LD_LIBRARY_PATH do cliente Steam.
    if std::env::var_os("CARGO_FEATURE_STEAM").is_some()
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux")
    {
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN");
    }

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        link_clang_rt_osx();
    }

    tauri_build::build()
}

/// macOS: o ONNX Runtime (CoreML) chama `___isPlatformVersionAtLeast` (checagens de `@available`),
/// símbolo do runtime do clang. O `rustc` linka com `-nodefaultlibs`, então esse runtime não entra,
/// e o símbolo só resolve pela `libSystem` quando o deployment target é o do SDK novo. O
/// `tauri build` usa `minimumSystemVersion` (13.4) como alvo, o linker esconde o símbolo da
/// `libSystem` e o link falha com `Undefined symbols ... ___isPlatformVersionAtLeast`.
/// Ligar o `libclang_rt.osx.a` explicitamente resolve em qualquer alvo.
fn link_clang_rt_osx() {
    let resource_dir = std::process::Command::new("clang")
        .arg("--print-resource-dir")
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string());
    let lib = resource_dir.map(|dir| std::path::Path::new(&dir).join("lib/darwin/libclang_rt.osx.a"));
    match lib {
        Some(path) if path.exists() => println!("cargo:rustc-link-arg={}", path.display()),
        _ => println!("cargo:warning=libclang_rt.osx.a não encontrado: o link do ORT/CoreML pode falhar"),
    }
}
