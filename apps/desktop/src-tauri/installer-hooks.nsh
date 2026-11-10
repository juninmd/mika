; Hooks do instalador NSIS (bundle.windows.nsis.installerHooks).
;
; O `ort` (feature `directml`) deixa `DirectML.dll` ao lado do exe em `target/release/`, mas o
; bundler do Tauri só empacota o exe e `bundle.resources`: sem este hook o instalador "direct" sai
; sem a DLL, o ONNX Runtime não registra o DirectML e STT/TTS caem para CPU (com `log::warn!` em
; `modules/inference/session.rs`, sem erro na tela). O CI (release.yml, job `direct`) aponta a DLL
; em MIKA_DIRECTML_DLL, lida em tempo de compilação do NSIS (`$%VAR%`). Sem a variável (build
; local) o instalador sai sem ela, como antes.

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
