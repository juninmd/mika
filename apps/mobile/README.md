# Mika Android (protótipo)

Protótipo de viabilidade. Não substitui nem altera o app desktop: tem crate Rust próprio
(`[workspace]` isolado, `Cargo.lock` próprio) e reaproveita só o motor VRM do renderer desktop.

## O que roda

- Avatar VRM (`VrmAvatar` do desktop) no WebView do Android
- Chat de texto via backend Rust (`chat`), em qualquer endpoint compatível com OpenAI
  (Ollama no PC pela rede, OpenAI, Groq...). A chave de API fica só na memória da sessão
- Boca anima pelo tamanho da resposta (sem TTS ainda)

## Build

Pré-requisitos: Android SDK + NDK, `rustup target add x86_64-linux-android aarch64-linux-android`.

```bash
export ANDROID_HOME=... NDK_HOME=$ANDROID_HOME/ndk/<versão>
pnpm --filter @mika/mobile android:build   # APK debug x86_64 (emulador)
```

No emulador, `10.0.2.2` é o PC: com `ollama serve` rodando, o padrão `http://10.0.2.2:11434/v1` funciona.

## Fora do protótipo

Voz (STT/TTS), memória, visão, bolha flutuante e qualquer módulo do backend desktop.
