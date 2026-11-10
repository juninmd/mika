//! VAD (Silero) sobre `ort`. Destrava `voice:vad-audio`, hoje recusado em `modules/voice.rs`.
//!
//! Porte fiel de `apps/desktop/src/main/services/voice/vad/silero-vad.ts` (a classe `SileroVad`
//! de lá é a fonte da verdade dos nomes de tensor, das dimensões e do formato de janela — não
//! foram inventados aqui). Esse arquivo TS continua sendo o que `NeuralVadAnalyzer`/
//! `UtteranceSpeechGate` usam hoje; portar o EventBus e o canal `voice:vad-audio` em si fica para
//! quem tiver `modules/voice.rs` no escopo.
//!
//! O download do modelo já está portado e verificado por SHA-256 em `modules/desktop.rs`
//! (`wizard-prepare-asset`, ramo `vad`) — o arquivo chega em `<data>/vad-cache/silero_vad.onnx`.
//! `load()` só abre esse arquivo; não baixa, não reverifica o hash.
//!
//! **Carregamento único, não um segundo download.** Dois `SileroVad` no boot já correram o rename
//! do modelo por baixo do ORT e deram EACCES (ver `docs/tauri-rust-migration.md`). Aqui a mesma
//! classe de corrida existiria se cada instância abrisse seu próprio `ort::Session` do mesmo
//! arquivo: `SHARED_SESSION` (um `tokio::sync::OnceCell` estático) garante que só a primeira
//! chamada de `load()` monta a sessão — as demais, concorrentes ou não, recebem o mesmo
//! `Arc<Mutex<Session>>`. O teste `concurrent_loads_share_one_session_not_two` prova isso
//! comparando o ponteiro.
//!
//! Sessão compartilhada, estado NÃO compartilhado: cada `SileroVad` guarda seu próprio `state`
//! (LSTM recorrente) e `context` (as últimas `CONTEXT_SAMPLES` amostras da janela anterior). São
//! streams de áudio independentes que só têm em comum os pesos do modelo — misturar os dois em
//! uma única instância global corromperia a detecção de todo mundo com o histórico de qualquer um.
//!
//! `Session::run` do `ort` é síncrono e pede `&mut self`; cada chamada roda dentro de
//! `tokio::task::spawn_blocking` para não travar o worker que também resolve `mika_invoke`
//! (decisão registrada em `modules/inference/mod.rs`), e o `Mutex` serializa o acesso à sessão
//! compartilhada entre chamadas concorrentes de instâncias diferentes.

use std::path::Path;
use std::sync::Arc;

use ort::session::Session;
use ort::value::Tensor;
use tokio::sync::{Mutex as AsyncMutex, OnceCell};

/// Uma janela = 32 ms a 16 kHz. Outro tamanho é rejeitado por `validate_frame`, nunca recortado
/// ou preenchido em silêncio.
pub const FRAME_SAMPLES: usize = 512;
/// O modelo espera o rabo da janela anterior colado à frente; sem isso a inferência degrada perto
/// de zero e o barge-in nunca dispara (mesma constante de `silero-vad.ts::CONTEXT_SAMPLES`).
pub const CONTEXT_SAMPLES: usize = 64;
/// Estado recorrente do Silero: `[2, batch=1, 128]` achatado.
const STATE_LEN: usize = 2 * 128;
const EXPECTED_SAMPLE_RATE: u32 = 16_000;
const EXPECTED_CHANNELS: u16 = 1;

/// Uma janela de áudio no único formato que este VAD aceita. `probability` rejeita qualquer outro
/// `sample_rate`/`channels`/tamanho com um erro explícito — nunca reamostra ou faz downmix em
/// silêncio (contrato do módulo `inference`).
#[derive(Clone, Copy)]
pub struct Pcm16Frame<'a> {
    pub samples: &'a [i16],
    pub sample_rate: u32,
    pub channels: u16,
}

fn validate_frame(frame: &Pcm16Frame<'_>) -> anyhow::Result<()> {
    if frame.sample_rate != EXPECTED_SAMPLE_RATE {
        anyhow::bail!(
            "VAD aceita só {EXPECTED_SAMPLE_RATE} Hz, recebeu {} Hz — sem reamostragem silenciosa",
            frame.sample_rate
        );
    }
    if frame.channels != EXPECTED_CHANNELS {
        anyhow::bail!(
            "VAD aceita só mono, recebeu {} canais — sem downmix silencioso",
            frame.channels
        );
    }
    if frame.samples.len() != FRAME_SAMPLES {
        anyhow::bail!(
            "VAD espera janelas de {FRAME_SAMPLES} amostras (32 ms @ {EXPECTED_SAMPLE_RATE} Hz), recebeu {}",
            frame.samples.len()
        );
    }
    Ok(())
}

fn pcm16_to_f32(samples: &[i16]) -> Vec<f32> {
    samples.iter().map(|&s| s as f32 / 32_768.0).collect()
}

/// Monta a janela que o modelo espera: contexto anterior + quadro novo.
fn with_context(context: &[f32], frame: &[f32]) -> Vec<f32> {
    let mut input = Vec::with_capacity(context.len() + frame.len());
    input.extend_from_slice(context);
    input.extend_from_slice(frame);
    input
}

/// O rabo do quadro vira o contexto do próximo. `frame` já passou por `validate_frame`, então tem
/// sempre `FRAME_SAMPLES` (> `CONTEXT_SAMPLES`) amostras — não há caso de preenchimento a tratar.
fn next_context(frame: &[f32]) -> Vec<f32> {
    frame[frame.len() - CONTEXT_SAMPLES..].to_vec()
}

fn zeroed_state() -> Vec<f32> {
    vec![0.0; STATE_LEN]
}

fn zeroed_context() -> Vec<f32> {
    vec![0.0; CONTEXT_SAMPLES]
}

/// Sessão ONNX compartilhada por todo o processo — ver o cabeçalho do arquivo. `OnceCell` colapsa
/// chamadas concorrentes de `load()` numa única carga; a lida com a sessão em si (`Session::run`
/// pede `&mut self`) fica serializada pelo `Mutex` por trás do `Arc`.
static SHARED_SESSION: OnceCell<Arc<AsyncMutex<Session>>> = OnceCell::const_new();

async fn shared_session(model_path: &Path) -> anyhow::Result<Arc<AsyncMutex<Session>>> {
    let path = model_path.to_path_buf();
    let session = SHARED_SESSION
        .get_or_try_init(move || async move {
            tokio::task::spawn_blocking(move || {
                // Uma janela de 32 ms num LSTM minúsculo: GPU só somaria cópia, e o pool padrão do
                // ORT acordaria todos os núcleos a cada janela.
                let (session, _) = super::session::load(super::accel::Model::Silero, &path, Some(1))?;
                Ok::<_, anyhow::Error>(Arc::new(AsyncMutex::new(session)))
            })
            .await
            .map_err(|join_err| anyhow::anyhow!("carga do VAD cancelada: {join_err}"))?
        })
        .await?;
    Ok(Arc::clone(session))
}

/// VAD neural (Silero) sobre uma janela PCM16 mono 16 kHz por vez.
#[derive(Debug)]
pub struct SileroVad {
    session: Arc<AsyncMutex<Session>>,
    state: Vec<f32>,
    context: Vec<f32>,
}

impl SileroVad {
    /// Abre (ou reusa, ver `shared_session`) a sessão Silero a partir do `.onnx` já baixado e
    /// verificado por `modules/desktop.rs`. Não baixa nada — modelo ausente é erro explícito, não
    /// um novo download disfarçado de carga.
    pub async fn load(model_path: &Path) -> anyhow::Result<Self> {
        if !model_path.is_file() {
            anyhow::bail!(
                "modelo do VAD ausente em {}; rode o wizard (mika:wizard-prepare-asset, ramo vad) antes de carregar",
                model_path.display()
            );
        }
        let session = shared_session(model_path).await?;
        Ok(Self {
            session,
            state: zeroed_state(),
            context: zeroed_context(),
        })
    }

    /// Probabilidade de fala na janela, em `[0, 1]`. Atualiza o estado recorrente e o contexto
    /// para a próxima chamada — chamadas sucessivas na MESMA instância formam um stream contínuo;
    /// instâncias diferentes nunca compartilham estado (só a sessão).
    ///
    /// Qualquer falha (do `ort` ou o task cancelado/em pânico) reseta o estado antes de propagar o
    /// erro: uma falha transitória não pode deixar o LSTM num estado estagnado que degrada a
    /// detecção em silêncio na próxima janela (mesma garantia de `silero-vad.ts::probability`).
    pub async fn probability(&mut self, frame: Pcm16Frame<'_>) -> anyhow::Result<f32> {
        validate_frame(&frame)?;

        let samples_f32 = pcm16_to_f32(frame.samples);
        let input = with_context(&self.context, &samples_f32);
        let state_in = self.state.clone();
        let session = Arc::clone(&self.session);

        let joined = tokio::task::spawn_blocking(move || run_silero(&session, input, state_in)).await;
        let outcome = match joined {
            Ok(result) => result,
            Err(join_err) => Err(anyhow::anyhow!("inferência do VAD cancelada: {join_err}")),
        };

        match outcome {
            Ok((probability, next_state)) => {
                self.state = next_state;
                self.context = next_context(&samples_f32);
                Ok(probability)
            }
            Err(err) => {
                self.reset();
                Err(err)
            }
        }
    }

    /// Zera estado recorrente e contexto — mesmo efeito de uma instância recém-carregada.
    pub fn reset(&mut self) {
        self.state = zeroed_state();
        self.context = zeroed_context();
    }

    #[cfg(test)]
    fn session_ptr(&self) -> *const AsyncMutex<Session> {
        Arc::as_ptr(&self.session)
    }
}

/// A chamada síncrona ao `ort` em si — roda dentro de `spawn_blocking` (ver `probability`), nunca
/// direto numa task async.
fn run_silero(session: &AsyncMutex<Session>, input: Vec<f32>, state: Vec<f32>) -> anyhow::Result<(f32, Vec<f32>)> {
    let mut session = session.blocking_lock();

    let input_len = input.len();
    let input_tensor = Tensor::from_array(([1usize, input_len], input))?;
    let state_tensor = Tensor::from_array(([2usize, 1usize, 128usize], state))?;
    let sr_tensor = Tensor::from_array(((), vec![i64::from(EXPECTED_SAMPLE_RATE)]))?;

    let outputs = session.run(ort::inputs![
        "input" => input_tensor,
        "state" => state_tensor,
        "sr" => sr_tensor,
    ])?;

    let (_, probability_data) = outputs["output"].try_extract_tensor::<f32>()?;
    let probability = *probability_data
        .first()
        .ok_or_else(|| anyhow::anyhow!("Silero não devolveu probabilidade"))?;

    let (_, next_state) = outputs["stateN"].try_extract_tensor::<f32>()?;

    Ok((probability, next_state.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // --- lógica pura, sem sessão nem rede -------------------------------------------------

    fn silent_frame(buf: &[i16]) -> Pcm16Frame<'_> {
        Pcm16Frame {
            samples: buf,
            sample_rate: EXPECTED_SAMPLE_RATE,
            channels: EXPECTED_CHANNELS,
        }
    }

    #[test]
    fn wrong_sample_rate_is_rejected_explicitly() {
        let buf = [0i16; FRAME_SAMPLES];
        let frame = Pcm16Frame {
            sample_rate: 8_000,
            ..silent_frame(&buf)
        };
        let err = validate_frame(&frame).unwrap_err().to_string();
        assert!(err.contains("16_000") || err.contains("16000"), "mensagem sem a taxa esperada: {err}");
    }

    #[test]
    fn wrong_channel_count_is_rejected_explicitly() {
        let buf = [0i16; FRAME_SAMPLES];
        let frame = Pcm16Frame {
            channels: 2,
            ..silent_frame(&buf)
        };
        let err = validate_frame(&frame).unwrap_err().to_string();
        assert!(err.contains("mono"), "mensagem sem citar mono: {err}");
    }

    #[test]
    fn wrong_frame_length_is_rejected_explicitly() {
        let buf = [0i16; FRAME_SAMPLES / 2];
        let err = validate_frame(&silent_frame(&buf)).unwrap_err().to_string();
        assert!(err.contains(&FRAME_SAMPLES.to_string()), "mensagem sem o tamanho esperado: {err}");
    }

    #[test]
    fn well_formed_frame_passes_validation() {
        let buf = [0i16; FRAME_SAMPLES];
        assert!(validate_frame(&silent_frame(&buf)).is_ok());
    }

    #[test]
    fn pcm16_to_f32_normalizes_into_unit_range() {
        let floats = pcm16_to_f32(&[i16::MIN, 0, i16::MAX]);
        assert!((floats[0] - (-1.0)).abs() < 1e-6, "MIN devia normalizar para -1.0, deu {}", floats[0]);
        assert_eq!(floats[1], 0.0);
        assert!(floats[2] > 0.999 && floats[2] < 1.0, "MAX devia ficar logo abaixo de 1.0, deu {}", floats[2]);
    }

    #[test]
    fn with_context_prepends_the_previous_tail() {
        let context = vec![1.0_f32; CONTEXT_SAMPLES];
        let frame = vec![2.0_f32; FRAME_SAMPLES];
        let input = with_context(&context, &frame);
        assert_eq!(input.len(), CONTEXT_SAMPLES + FRAME_SAMPLES);
        assert_eq!(&input[..CONTEXT_SAMPLES], context.as_slice());
        assert_eq!(&input[CONTEXT_SAMPLES..], frame.as_slice());
    }

    #[test]
    fn next_context_keeps_only_the_frame_tail() {
        let frame: Vec<f32> = (0..FRAME_SAMPLES).map(|i| i as f32).collect();
        let tail = next_context(&frame);
        assert_eq!(tail.len(), CONTEXT_SAMPLES);
        assert_eq!(tail.as_slice(), &frame[FRAME_SAMPLES - CONTEXT_SAMPLES..]);
    }

    #[test]
    fn zeroed_state_and_context_have_the_shapes_silero_expects() {
        assert_eq!(zeroed_state().len(), 2 * 1 * 128);
        assert!(zeroed_state().iter().all(|&v| v == 0.0));
        assert_eq!(zeroed_context().len(), CONTEXT_SAMPLES);
    }

    #[tokio::test]
    async fn load_fails_loudly_when_the_model_file_is_missing() {
        let missing = std::env::temp_dir().join("mika-vad-test-definitely-missing.onnx");
        let _ = std::fs::remove_file(&missing);
        let err = SileroVad::load(&missing).await.unwrap_err().to_string();
        assert!(err.contains("ausente"), "erro devia dizer que o modelo está ausente: {err}");
    }

    // --- modelo real: mesmo Silero que a produção baixa, mesmo fixture de voz do E2E -------
    //
    // O `.onnx` é RASTREADO no repositório (2,3 MB, MIT, `*.onnx binary` no `.gitattributes`) em
    // vez de baixado na hora. A primeira versão deste helper chamava `curl`: o teste passava aqui e
    // vermelhava em qualquer máquina sem rede — e `test:rust` roda dentro de `pnpm gates`, que é a
    // única verdade deste projeto enquanto o CI está recusado por billing. Gate que depende de rede
    // falha por motivo alheio à mudança.
    //
    // Pular quando o arquivo falta seria pior ainda: viraria verde otimista, a classe de defeito que
    // os gates daqui existem para impedir. Então falta de fixture é falha alta, com o caminho no
    // texto. O hash continua conferido porque um `.onnx` corrompido em disco daria erro do `ort`
    // lá adiante, longe da causa.
    //
    // Isto NÃO reimplementa `modules/desktop.rs::wizard-prepare-asset`, que é o download de
    // produção e continua sendo quem busca o modelo para o app de verdade.

    const REAL_MODEL_SHA256: &str = "1a153a22f4509e292a94e67d6f9b85e8deb25b4988682b7e174c65279d8788e3";

    fn sha256_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hex::encode(hasher.finalize())
    }

    fn ensure_real_model() -> PathBuf {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../e2e/fixtures/models/silero_vad.onnx");

        let bytes = std::fs::read(&path).unwrap_or_else(|e| {
            panic!("fixture do Silero ausente em {}: {e}. É arquivo rastreado no repositório.", path.display())
        });
        assert_eq!(sha256_hex(&bytes), REAL_MODEL_SHA256, "fixture do Silero em {} não bate com o SHA-256 esperado", path.display());
        path
    }

    /// Mesma gravação real (fala em pt-BR, 16 kHz mono) que o E2E de voz usa — não é sintética.
    fn load_real_speech_fixture() -> Vec<i16> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../e2e/fixtures/voice/fala-usuario-pt-br.wav");
        let mut reader =
            hound::WavReader::open(&path).unwrap_or_else(|e| panic!("abrir fixture de voz real {}: {e}", path.display()));
        let spec = reader.spec();
        assert_eq!(spec.sample_rate, EXPECTED_SAMPLE_RATE, "fixture não está em 16 kHz");
        assert_eq!(spec.channels, 1, "fixture não é mono");
        reader.samples::<i16>().map(|s| s.expect("amostra PCM16 válida no fixture")).collect()
    }

    fn window(samples: &[i16], index: usize) -> &[i16] {
        &samples[index * FRAME_SAMPLES..(index + 1) * FRAME_SAMPLES]
    }

    fn frame(samples: &[i16], index: usize) -> Pcm16Frame<'_> {
        silent_frame(window(samples, index))
    }

    /// Janela de maior e de menor energia RMS na gravação — dinâmico em vez de índice fixo, então
    /// não depende de conhecer o conteúdo exato do fixture.
    fn loudest_and_quietest_window(samples: &[i16]) -> (usize, usize) {
        let windows = samples.len() / FRAME_SAMPLES;
        let energy = |w: &[i16]| -> f64 {
            let sum_sq: f64 = w.iter().map(|&s| f64::from(s).powi(2)).sum();
            (sum_sq / w.len() as f64).sqrt()
        };
        let mut loudest = (0usize, f64::MIN);
        let mut quietest = (0usize, f64::MAX);
        for i in 0..windows {
            let e = energy(window(samples, i));
            if e > loudest.1 {
                loudest = (i, e);
            }
            if e < quietest.1 {
                quietest = (i, e);
            }
        }
        (loudest.0, quietest.0)
    }

    #[tokio::test]
    async fn concurrent_loads_share_one_session_not_two() {
        let model_path = ensure_real_model();
        let (a, b) = tokio::join!(SileroVad::load(&model_path), SileroVad::load(&model_path));
        let (a, b) = (a.expect("carregar A"), b.expect("carregar B"));
        assert_eq!(
            a.session_ptr(),
            b.session_ptr(),
            "duas cargas concorrentes deviam colapsar numa sessão só, não abrir duas"
        );
    }

    #[tokio::test]
    async fn detects_speech_probability_higher_in_the_loudest_real_window() {
        let model_path = ensure_real_model();
        let samples = load_real_speech_fixture();
        let (loud_idx, quiet_idx) = loudest_and_quietest_window(&samples);

        let mut loud_vad = SileroVad::load(&model_path).await.expect("carregar VAD real");
        let loud_probability = loud_vad.probability(frame(&samples, loud_idx)).await.expect("inferência real (loud)");

        let mut quiet_vad = SileroVad::load(&model_path).await.expect("carregar VAD real");
        let quiet_probability = quiet_vad.probability(frame(&samples, quiet_idx)).await.expect("inferência real (quiet)");

        assert!(
            (0.0..=1.0).contains(&loud_probability) && (0.0..=1.0).contains(&quiet_probability),
            "probabilidade fora de [0,1]: loud={loud_probability} quiet={quiet_probability}"
        );
        assert!(
            loud_probability > quiet_probability,
            "janela mais energética ({loud_probability}) devia superar a mais silenciosa ({quiet_probability})"
        );
    }

    /// A GPU tem de dar o mesmo número que a CPU, e numa máquina sem EP de GPU a queda para CPU
    /// não pode virar erro: é o que separa "GPU ligada" de "voz morta" (`inference/session.rs`).
    #[test]
    fn gpu_session_matches_cpu_on_the_real_model() {
        let model_path = ensure_real_model();
        let samples = load_real_speech_fixture();
        let (loud, _) = loudest_and_quietest_window(&samples);
        let input = with_context(&zeroed_context(), &pcm16_to_f32(window(&samples, loud)));
        let run = |device| {
            let plan = super::super::accel::Plan { device, reason: super::super::accel::Reason::UserGpu };
            let (session, _) = super::super::session::build(&model_path, plan, Some(1)).expect("sessão do Silero");
            run_silero(&AsyncMutex::new(session), input.clone(), zeroed_state()).expect("inferência").0
        };
        let (cpu, gpu) = (run(super::super::session::Device::Cpu), run(super::super::session::Device::Gpu));
        assert!((cpu - gpu).abs() < 1e-3, "GPU divergiu da CPU: {gpu} vs {cpu}");
    }

    #[tokio::test]
    async fn recurrent_state_changes_the_next_window_probability() {
        let model_path = ensure_real_model();
        let samples = load_real_speech_fixture();
        assert!(samples.len() >= FRAME_SAMPLES * 2, "fixture curta demais para duas janelas");

        let mut sequential = SileroVad::load(&model_path).await.expect("carregar VAD real");
        sequential.probability(frame(&samples, 0)).await.expect("primeira janela");
        let with_state = sequential.probability(frame(&samples, 1)).await.expect("segunda janela, com estado");

        let mut isolated = SileroVad::load(&model_path).await.expect("carregar VAD real");
        let without_state = isolated.probability(frame(&samples, 1)).await.expect("segunda janela, isolada");

        assert_ne!(
            with_state, without_state,
            "a mesma janela devia dar probabilidades diferentes com e sem o estado da janela anterior"
        );
    }
}
