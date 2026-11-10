mod bridge;
mod capture;
mod config;
#[cfg(feature = "e2e")]
mod e2e_host;
mod events;
mod feature_health;
mod feature_registry;
mod images;
#[cfg(test)]
mod ipc_validation_audit;
mod live2d_protocol;
mod modules;
mod native;
mod permissions;
mod power;
mod protocol;
mod shell_actions;
mod log_level;
mod shutdown;
mod fullscreen;
mod wizard_watch;
mod shortcuts;
mod state;
#[cfg(feature = "steam")]
mod steam;
mod tray;
mod tray_status;
mod updater;
mod windows;

use std::sync::atomic::Ordering;

use tauri::{Manager, RunEvent, WindowEvent};

use state::ShellState;

/// Steam atualiza pelo próprio cliente: nessa build o plugin nem é registrado.
#[cfg(not(feature = "steam"))]
fn with_updater(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder.plugin(tauri_plugin_updater::Builder::new().build())
}

#[cfg(feature = "steam")]
fn with_updater(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // WebKitGTK com o renderer DMA-BUF cai em janela branca/transparente quebrada em vários drivers
    // (NVIDIA, Wayland, Steam Deck). Vale o caminho mais lento e estável; quem definir a variável manda.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    // Antes de tudo: fora da Steam, o cliente relança o jogo (licença + SteamPipe) e este processo sai.
    #[cfg(feature = "steam")]
    if steam::relaunch_through_steam_if_needed() {
        return;
    }
    // Panic em task tokio/spawn_blocking mata só a task; sem hook ninguém saberia.
    std::panic::set_hook(Box::new(|info| {
        eprintln!("PANIC: {info}");
        log::error!("PANIC: {info}");
        // Panic na thread principal pode derrubar o processo antes de o escritor gravar esta linha.
        modules::system::log_files::flush();
    }));
    with_updater(tauri::Builder::default())
        /*
         * PRIMEIRO plugin, porque tudo que vem depois já pode logar.
         *
         * O crate `log` sem logger registrado DESCARTA a mensagem — não é "vai para stderr por
         * padrão", é silêncio. Sem isto, as chamadas `log::error!` deste shell eram código
         * morto, incluindo "backend não subiu" e "permissão de mídia negada para origem de
         * terceiro": o app subia com a janela escondida e o backend fora do ar sem imprimir uma
         * linha em lugar nenhum. É `check:rust-logger` que impede isso de voltar.
         *
         * Stderr, nunca Stdout: o stdout DESTE processo não é protocolo (o do sidecar é), mas
         * manter os dois lados na mesma disciplina evita a próxima confusão.
         */
        .plugin(
            tauri_plugin_log::Builder::new()
                // Sem isto os alvos padrão (Stdout + LogDir) somam aos daqui: linha dupla no
                // terminal e um `Mika.log` sem redação de segredo.
                .clear_targets()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stderr,
                ))
                // Ring buffer em memória do painel de Logs (Configurações) E os dois arquivos de
                // `specs/10-observability.md` ("Dois públicos, dois tiers"): `modules::system::
                // log_buffer` redige segredo, resolve `audience`/`traceId` e devolve o `LogRecord`
                // cru, que `log_files::enqueue` empurra pro sink de disco (nunca bloqueia esta
                // chamada — ver o cabeçalho de `log_files.rs`). Substituiu o antigo target
                // `TargetKind::LogDir { file_name: "mika-shell" }`: aquele escrevia direto do
                // `fern`, sem passar pela redação, e sem par human/ai — os dois arquivos abaixo
                // fecham as duas lacunas de uma vez, a partir do MESMO registro.
                .target(tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Dispatch(
                    tauri_plugin_log::fern::Dispatch::new().chain(tauri_plugin_log::fern::Output::call(|record| {
                        let log_record = modules::system::log_buffer::push_from_log_record(record);
                        modules::system::log_files::enqueue(log_record);
                    })),
                )))
                // Dependencies stay at info; our crate may go down to debug, and `log_level` sets
                // the effective filter from `system.logLevel`.
                .level(log::LevelFilter::Info)
                .level_for("mika_desktop_lib", log::LevelFilter::Debug)
                .build(),
        )
        // Uma instância só: duas Mikas disputando o mesmo perfil corrompem o store.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window(windows::MAIN) {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // Serve `<app_data_dir>/live2d/` para o webview — a única fonte de modelos Live2D em
        // produção, já que as amostras de terceiro saem do instalador (LICENSE-ASSETS.md). Ver
        // `live2d_protocol.rs` para por que um esquema custom, e não `assetProtocol.scope`
        // (estático, não acompanha `MIKA_PROFILE_DIR`).
        .register_uri_scheme_protocol(live2d_protocol::SCHEME, live2d_protocol::handler)
        .manage(ShellState::new())
        .invoke_handler(tauri::generate_handler![
            bridge::mika_invoke,
            bridge::mika_send,
            bridge::mika_send_pcm
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            // O plugin libera Debug para o nosso crate (o painel de Logs pode pedi-lo), o que deixa o
            // filtro global em Debug até `log_level::spawn` ler `system.logLevel`. Em release o
            // padrão é Info desde o primeiro instante: debug só por escolha explícita no painel.
            if !cfg!(debug_assertions) {
                log::set_max_level(log::LevelFilter::Info);
            }

            // Dois arquivos de `specs/10-observability.md` ("Persistência"): `logs/mika-human.log`
            // (tier humano) e `logs/mika-ai.ndjson` (tier IA, tudo). `state::app_data_dir` resolve
            // o mesmo diretório que todo outro sink deste processo, respeitando
            // `MIKA_PROFILE_DIR` — sem isto o E2E gravaria log no perfil real do usuário. Falha ao
            // resolver o diretório não aborta o boot: os dois arquivos ficam sem sink (ring buffer
            // e stderr continuam funcionando), e o motivo cai no próprio log.
            match state::app_data_dir(&handle) {
                Ok(dir) => modules::system::log_files::spawn(dir),
                Err(error) => log::error!("logs: não consegui resolver o diretório de dados ({error})"),
            }

            // Raiz de recursos de `prompts/` para quem monta o `system.md` rico sem `AppHandle`
            // (`brain::turn::run_turn_llm` — assinatura fixa, não pode ganhar o parâmetro). Sem
            // esta chamada, build empacotado cai no fallback de dev (inexistente fora da árvore de
            // source) e falha alto — ver o cabeçalho de `modules/chat/prompt.rs`.
            modules::chat::init_prompts_root(&handle);

            // As tools que precisam do `ConfigStore` — ver `modules::register_config_bound_tools`.
            // Antes da janela: o catálogo tem de estar cheio quando o primeiro turno pedir a lista.
            modules::register_config_bound_tools(&handle);

            // Rich Presence, conquistas e pump de callbacks — só na build Steam, e só se a Steam responder.
            #[cfg(feature = "steam")]
            steam::start(&handle);

            // Servidores MCP do usuário. Não bloqueia o boot: `boot` dispara o connect em
            // background e sai. Com `mcp.enabled` em `false` (o default nos dois lados) ele nem
            // lê o `mcp.json` — nenhum processo de terceiro sobe sem o usuário ter ligado.
            modules::mcp::boot(&handle);
            modules::voice::prewarm_local_stt(&handle);
            modules::voice::prewarm_local_tts(&handle);
            // Modelo do chat carregado antes do primeiro turno — ver `brain::ollama_warmup`.
            if let Ok(store) = state::config_store(&handle) {
                modules::brain::ollama_warmup::spawn(store);
            }

            #[cfg(feature = "e2e")]
            e2e_host::start(&handle);

            windows::create_main(&handle)?;
            tray::setup(&handle)?;
            tray_status::spawn(handle.clone());
            // F9, Ctrl+Space e Ctrl+Shift+M. O Electron os registra no boot; o shell Tauri não
            // registrava ninguém, e as três teclas eram mortas sem um único erro.
            shortcuts::register_app_shortcuts(&handle);

            // A janela nasce escondida, e quem decide o que mostrar é esta linha: onboarding
            // pendente abre o wizard, senão revela o avatar. Era o sidecar quem decidia
            // (`sidecar/index.ts::decideWizard`); com ele apagado, sem este porte todo boot cairia
            // no prazo de segurança abaixo e o wizard de primeiro uso nunca mais abriria.
            shell_actions::decide_boot(&handle);
            wizard_watch::spawn(&handle);
            fullscreen::spawn(&handle);
            log_level::spawn(&handle);
            modules::system::log_stream::spawn(&handle);

            // Rede de segurança: se a decisão acima falhar, a janela aparece assim mesmo em 8s com
            // o motivo no log. Janela que nunca aparece é a falha mais cara de diagnosticar.
            shell_actions::reveal_after_timeout(&handle, 8);

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let quitting = app.state::<ShellState>().quitting.load(Ordering::Relaxed);

                // Fechar a janela do avatar esconde, não encerra — igual ao Electron.
                if window.label() == windows::MAIN && !quitting {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("falha ao montar o app")
        .run(|app, event| {
            // Saída que não passou pelo `ExitRequested` (ex.: `app.exit(0)` do fim do desligamento, SO).
            if matches!(event, RunEvent::Exit) {
                modules::system::log_files::flush();
            }
            if let RunEvent::ExitRequested { api, .. } = event {
                let shell = app.state::<ShellState>();
                shell.quitting.store(true, Ordering::Relaxed);
                if shell.shutdown_started.swap(true, Ordering::Relaxed) {
                    return;
                }

                // Segura o encerramento até o backend rodar os shutdown hooks — mesmo
                // contrato do `createQuitGate`. Sem isso o store salva pela metade.
                api.prevent_exit();

                // Um disparo só, para todo loop de fundo assinado via `shutdown::subscribe()`
                // (`modules::lifecycle_shutdown_gate` prova quem assina) — ver o cabeçalho de
                // `shutdown.rs` para por que `watch` e não `tokio-util`.
                shutdown::trigger();

                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    // "O que ficou estabelecido nessa conversa" (session_accumulator.rs) vive só no
                    // processo — sem flush aqui, morre com ele. Este é o único "fim de sessão" que o
                    // Rust tem hoje (ver o cabeçalho daquele arquivo para o porquê de não haver um
                    // evento de sessão fechando dentro de post_turn).
                    if let Ok(mem_store) = modules::memory::memory_store(&app) {
                        let recording_allowed = crate::state::config_store(&app)
                            .map(|store| modules::memory::memory_recording_allowed(&store))
                            .unwrap_or(false);
                        // O flush embeda (o provedor pode demorar até 20 s ou estar fora do ar): fora do
                        // worker async e com teto, para não segurar o encerramento do app.
                        let flush = tauri::async_runtime::spawn_blocking(move || {
                            modules::brain::post_turn::session_accumulator::accumulator().flush(mem_store, recording_allowed)
                        });
                        match shutdown::within("gravar memória de sessão", shutdown::STEP_TIMEOUT, async { flush.await }).await {
                            Some(Ok(Err(error))) => log::warn!("memória de sessão não persistida no encerramento: {error}"),
                            Some(Err(error)) => log::warn!("flush da memória de sessão abortou: {error}"),
                            _ => {}
                        }
                    }
                    // Servidor MCP é processo do SO filho deste, e dropar o `RunningService` é o
                    // que o mata. Sair sem isto deixa processo órfão.
                    // Com teto: um servidor que não responde ao fechar não segura o app aberto.
                    shutdown::within("desconectar MCP", shutdown::STEP_TIMEOUT, async { modules::mcp::disconnect_all().await }).await;
                    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                    // Lote final de log (a razão da saída incluída) em disco antes de o processo morrer.
                    let _ = tauri::async_runtime::spawn_blocking(modules::system::log_files::flush).await;
                    app.exit(0);
                });
            }
        });
}
