; Hooks do instalador NSIS (bundle.windows.nsis.installerHooks).
;
; O `ort` (feature `directml`) deixa `DirectML.dll` ao lado do exe em `target/release/`, mas o
; bundler do Tauri só empacota o exe e `bundle.resources`: sem este hook o instalador "direct" sai
; sem a DLL, o ONNX Runtime não registra o DirectML e STT/TTS caem para CPU (com `log::warn!` em
; `modules/inference/session.rs`, sem erro na tela). O CI (release.yml, job `direct`) aponta a DLL
; em MIKA_DIRECTML_DLL, lida em tempo de compilação do NSIS (`$%VAR%`). Sem a variável (build
; local) o instalador sai sem ela, como antes.

;
; PREINSTALL: o auto update (`tauri-plugin-updater`) dispara o instalador e só DEPOIS encerra o
; processo da Mika, e o encerramento não é instantâneo (DLLs do ONNX/DirectML, threads de áudio).
; O `CheckIfAppIsRunning` do template roda logo em seguida, acha o exe ainda saindo, e o
; `TerminateProcess` num processo que já está terminando volta ACCESS_DENIED: o instalador aborta
; com "Failed to kill Mika. Please close it first then try again". Aqui esperamos o processo sair
; (até ~15 s); passou de 10 s, `taskkill /F /T` derruba a árvore inteira (app + filhos).

!macro NSIS_HOOK_PREINSTALL
  StrCpy $R1 0
  mika_wait_exit:
    nsis_tauri_utils::FindProcess "${MAINBINARYNAME}.exe"
    Pop $R0
    ${If} $R0 = 0
    ${AndIf} $R1 < 30
      IntOp $R1 $R1 + 1
      ${If} $R1 = 20
        nsExec::Exec 'taskkill /F /T /IM "${MAINBINARYNAME}.exe"'
        Pop $R0
      ${EndIf}
      Sleep 500
      Goto mika_wait_exit
    ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  !if "$%MIKA_DIRECTML_DLL%" != ""
    SetOutPath "$INSTDIR"
    File "$%MIKA_DIRECTML_DLL%"
  !endif
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; O desinstalador só remove o que o bundler conhece; sem isto a pasta de instalação sobra.
  Delete "$INSTDIR\DirectML.dll"
!macroend
