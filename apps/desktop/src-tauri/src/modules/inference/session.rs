//! Construtor ÚNICO de sessão ONNX do app (Silero, Whisper, Nemotron, Supertonic, e5). Antes cada
//! um chamava `Session::builder(env)?.commit_from_file()` cru: CPU só, threads no padrão do ORT (todos
//! os núcleos, com spin), e a GPU parada durante STT e TTS.
//!
//! **Quem vai para a GPU é decisão de [`super::accel`]** (por carga, preferência e governador);
//! aqui só se executa o [`accel::Plan`] e se reporta o que de fato aconteceu.
//!
//! **GPU com queda para CPU, nunca erro.** Cada EP de GPU compilado e aceito pelo ORT é tentado em
//! ordem, registrado com `error_on_failure` — sem isso o ORT seguia na CPU em silêncio e a leitura
//! afirmava GPU. O operador sem suporte (ex.: MatMulNBits int4 no DirectML) só estoura no COMMIT, por
//! isso a tentativa cobre registro + commit. Todos falharam: CPU, com o erro na leitura ao vivo.
//!
//! **EPs por SO** (cargo features em `Cargo.toml`):
//! - Windows: DirectML sempre (o binário do ORT baixado já é o build DirectML; roda em NVIDIA, AMD e
//!   Intel sem toolkit nenhum). `gpu-cuda`/`gpu-tensorrt` trocam o download pelo build CUDA 13 e
//!   tentam TensorRT → CUDA antes do DirectML.
//! - macOS: CoreML (Apple Silicon; formato MLProgram, o que cobre mais operadores).
//! - Linux: CPU por padrão (o binário padrão do ORT não tem EP de GPU). `gpu-cuda`/`gpu-tensorrt`
//!   (NVIDIA com CUDA 13 instalado) ou `gpu-webgpu` (Vulkan via Dawn — AMD/Intel/NVIDIA) ligam GPU.
//!   `gpu-webgpu` é SÓ Linux: no Windows o ORT não publica um build DirectML+WebGPU.
//!
//! **Opções de sessão:** otimização de grafo `All` explícita (é o padrão do ORT, fica escrito);
//! arena de memória da CPU no padrão (ligada — modelos longevos com forma variável reusam blocos);
//! sem spin de threads ociosas (queimava núcleos entre falas). IO binding não entra: o único laço
//! que devolveria tensores à GPU a cada passo é o denoiser do Supertonic, e o custo por passo é o
//! grafo, não a cópia de um latente de alguns KB.

use ort::session::builder::{GraphOptimizationLevel, SessionBuilder};
use ort::session::Session;
use std::path::Path;
use std::sync::OnceLock;

use super::accel::{self, Model, Plan, Reason, Report};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Device {
    /// GPU quando este build tem EP para ela; CPU caso contrário ou se todo EP falhar.
    Gpu,
    Cpu,
}

/// Uma carga de modelo (uma ou mais sessões, ex.: os três grafos do Supertonic). Ao sair de escopo
/// publica o [`Report`] agregado: GPU só se TODO grafo conseguiu GPU.
pub struct Load {
    model: Model,
    plan: Plan,
    threads: Option<usize>,
    report: Option<Report>,
}

impl Load {
    /// `threads` limita o pool da CPU (o LLM local e o jogo disputam os mesmos núcleos); `None`
    /// mantém o padrão do ORT.
    pub fn new(model: Model, threads: Option<usize>) -> Self {
        Self { model, plan: accel::plan(model), threads, report: None }
    }

    /// O plano desta carga — os caches guardam `plan().device` para recarregar quando ele mudar.
    pub fn plan(&self) -> Plan {
        self.plan
    }

    pub fn session(&mut self, path: &Path) -> ort::Result<Session> {
        let (session, report) = build(path, self.plan, self.threads)?;
        // Um grafo que caiu para CPU define a leitura do modelo inteiro.
        if self.report.as_ref().is_none_or(|r| r.error.is_none()) {
            self.report = Some(report);
        }
        Ok(session)
    }
}

impl Drop for Load {
    fn drop(&mut self) {
        if let Some(report) = self.report.take() {
            accel::record(self.model, report);
        }
    }
}

/// Uma sessão só — atalho de [`Load`] para modelo de um grafo.
pub fn load(model: Model, path: &Path, threads: Option<usize>) -> ort::Result<(Session, Plan)> {
    let mut load = Load::new(model, threads);
    Ok((load.session(path)?, load.plan()))
}

pub(super) fn build(path: &Path, plan: Plan, threads: Option<usize>) -> ort::Result<(Session, Report)> {
    let mut error = None;
    if plan.device == Device::Gpu {
        match first_ok(gpu_providers(), |ep| {
            let mut builder = ep.attach(cpu_builder(threads)?, path)?;
            builder.commit_from_file(path)
        }) {
            Ok((session, ep)) => {
                log::info!("ONNX {} em {}", file_name(path), ep.name());
                return Ok((session, Report { requested: Device::Gpu, provider: ep.name(), reason: plan.reason, error: None }));
            }
            Err(errors) if !errors.is_empty() => {
                let joined = errors.join("; ");
                log::warn!("ONNX {} sem GPU, caindo para CPU: {joined}", file_name(path));
                error = Some(joined);
            }
            Err(_) => {}
        }
    }
    let session = cpu_builder(threads)?.commit_from_file(path)?;
    log::info!("ONNX {} em CPU ({})", file_name(path), plan.reason.as_str());
    let reason = if error.is_some() { Reason::GpuFailed } else { plan.reason };
    Ok((session, Report { requested: plan.device, provider: "CPU", reason, error }))
}

/// Tenta cada candidato em ordem; o primeiro `Ok` vence. `Err` traz um erro por candidato tentado
/// (vazio = nenhum candidato).
fn first_ok<C: Copy + Named, T>(candidates: &[C], mut attempt: impl FnMut(C) -> ort::Result<T>) -> Result<(T, C), Vec<String>> {
    let mut errors = Vec::new();
    for &candidate in candidates {
        match attempt(candidate) {
            Ok(value) => return Ok((value, candidate)),
            Err(e) => errors.push(format!("{}: {e}", candidate.name())),
        }
    }
    Err(errors)
}

trait Named {
    fn name(self) -> &'static str;
}

/// Ambiente ORT do processo. Desde a rc.15 todo `Session::builder` exige um `&Environment`, e a
/// crate só guarda uma referência fraca: sem esta posse o ambiente cairia e a próxima sessão falharia.
fn environment() -> ort::Result<&'static ort::environment::Environment> {
    static ENV: std::sync::OnceLock<ort::environment::Environment> = std::sync::OnceLock::new();
    if let Some(env) = ENV.get() {
        return Ok(env);
    }
    let built = ort::init().build()?;
    Ok(ENV.get_or_init(|| built))
}

fn cpu_builder(threads: Option<usize>) -> ort::Result<SessionBuilder> {
    let builder = Session::builder(environment()?)?.with_optimization_level(GraphOptimizationLevel::All)?.with_intra_op_spinning(false)?;
    match threads {
        Some(threads) => Ok(builder.with_intra_threads(threads)?),
        None => Ok(builder),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// Os EPs de GPU compilados neste build. Em Linux sem feature de GPU a lista é vazia (e o enum,
/// inabitado).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gpu {
    #[cfg(feature = "gpu-tensorrt")]
    TensorRt,
    #[cfg(feature = "gpu-cuda")]
    Cuda,
    #[cfg(target_os = "windows")]
    DirectMl,
    #[cfg(target_os = "macos")]
    CoreMl,
    #[cfg(feature = "gpu-webgpu")]
    WebGpu,
}

impl Named for Gpu {
    fn name(self) -> &'static str {
        match self {
            #[cfg(feature = "gpu-tensorrt")]
            Self::TensorRt => "TensorRT",
            #[cfg(feature = "gpu-cuda")]
            Self::Cuda => "CUDA",
            #[cfg(target_os = "windows")]
            Self::DirectMl => "DirectML",
            #[cfg(target_os = "macos")]
            Self::CoreMl => "CoreML",
            #[cfg(feature = "gpu-webgpu")]
            Self::WebGpu => "WebGPU",
        }
    }
}

impl Gpu {
    /// Ordem de preferência: o mais rápido primeiro, o que roda em qualquer placa por último.
    fn compiled() -> Vec<Gpu> {
        #[allow(unused_mut)]
        let mut eps = Vec::new();
        #[cfg(feature = "gpu-tensorrt")]
        eps.push(Gpu::TensorRt);
        #[cfg(feature = "gpu-cuda")]
        eps.push(Gpu::Cuda);
        #[cfg(target_os = "windows")]
        eps.push(Gpu::DirectMl);
        #[cfg(target_os = "macos")]
        eps.push(Gpu::CoreMl);
        #[cfg(feature = "gpu-webgpu")]
        eps.push(Gpu::WebGpu);
        eps
    }

    /// O ORT carregado foi compilado com este EP? (Um `libonnxruntime` CPU com `gpu-cuda` diz não.)
    fn available(self) -> bool {
        #[allow(unused_imports)]
        use ort::ep::ExecutionProvider;
        match self {
            #[cfg(feature = "gpu-tensorrt")]
            Self::TensorRt => ort::ep::TensorRT::is_available().unwrap_or(false),
            #[cfg(feature = "gpu-cuda")]
            Self::Cuda => ort::ep::CUDA::is_available().unwrap_or(false),
            #[cfg(target_os = "windows")]
            Self::DirectMl => ort::ep::DirectML::is_available().unwrap_or(false),
            #[cfg(target_os = "macos")]
            Self::CoreMl => ort::ep::CoreML::is_available().unwrap_or(false),
            #[cfg(feature = "gpu-webgpu")]
            Self::WebGpu => ort::ep::WebGPU::is_available().unwrap_or(false),
        }
    }

    #[allow(unused_variables)]
    fn attach(self, builder: SessionBuilder, model: &Path) -> ort::Result<SessionBuilder> {
        match self {
            // Sem cache, o TensorRT recompila o engine a cada carga (minutos no primeiro uso).
            #[cfg(feature = "gpu-tensorrt")]
            Self::TensorRt => {
                let cache = model.parent().unwrap_or(model).join("tensorrt-cache").to_string_lossy().into_owned();
                let ep = ort::ep::TensorRT::default()
                    .with_engine_cache(true)
                    .with_engine_cache_path(&cache)
                    .with_timing_cache(true)
                    .with_timing_cache_path(&cache);
                Ok(builder.with_execution_providers([ep.build().error_on_failure()])?)
            }
            #[cfg(feature = "gpu-cuda")]
            Self::Cuda => Ok(builder.with_execution_providers([ort::ep::CUDA::default().build().error_on_failure()])?),
            // DirectML exige memory pattern desligado e execução sequencial (documentação do EP).
            // `HighPerformance`: num notebook híbrido, a GPU dedicada, não a integrada (adapter 0).
            #[cfg(target_os = "windows")]
            Self::DirectMl => {
                let ep = ort::ep::DirectML::default().with_performance_preference(ort::ep::directml::PerformancePreference::HighPerformance);
                Ok(builder
                    .with_memory_pattern(false)?
                    .with_parallel_execution(false)?
                    .with_execution_providers([ep.build().error_on_failure()])?)
            }
            #[cfg(target_os = "macos")]
            Self::CoreMl => {
                let ep = ort::ep::CoreML::default().with_model_format(ort::ep::coreml::ModelFormat::MLProgram);
                Ok(builder.with_execution_providers([ep.build().error_on_failure()])?)
            }
            #[cfg(feature = "gpu-webgpu")]
            Self::WebGpu => Ok(builder.with_execution_providers([ort::ep::WebGPU::default().build().error_on_failure()])?),
        }
    }
}

/// Os EPs de GPU compilados E aceitos pelo ORT carregado — consultado uma vez por processo.
fn gpu_providers() -> &'static [Gpu] {
    static AVAILABLE: OnceLock<Vec<Gpu>> = OnceLock::new();
    AVAILABLE.get_or_init(|| Gpu::compiled().into_iter().filter(|ep| ep.available()).collect())
}

pub fn gpu_available() -> bool {
    !gpu_providers().is_empty()
}

/// O primeiro EP de GPU que as cargas vão tentar; `None` = este build só tem CPU.
pub fn gpu_provider_name() -> Option<&'static str> {
    gpu_providers().first().map(|ep| ep.name())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Fake(&'static str);

    impl Named for Fake {
        fn name(self) -> &'static str {
            self.0
        }
    }

    fn fail() -> ort::Error {
        ort::Error::new("E_INVALIDARG")
    }

    /// TensorRT sem engine → CUDA → DirectML: o primeiro EP que commita vence, e o erro de quem
    /// falhou antes não some.
    #[test]
    fn gpu_providers_are_tried_in_order_until_one_commits() {
        let eps = [Fake("TensorRT"), Fake("CUDA"), Fake("DirectML")];
        let got = first_ok(&eps, |ep| if ep.0 == "TensorRT" { Err(fail()) } else { Ok(ep.0) });
        assert_eq!(got, Ok(("CUDA", Fake("CUDA"))));
    }

    #[test]
    fn every_failure_is_kept_for_the_readout_when_all_providers_fail() {
        let eps = [Fake("CUDA"), Fake("DirectML")];
        let errors = first_ok(&eps, |_| Err::<(), _>(fail())).unwrap_err();
        assert_eq!(errors.len(), 2);
        assert!(errors[0].starts_with("CUDA: ") && errors[1].starts_with("DirectML: "), "{errors:?}");
    }

    fn silero() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../e2e/fixtures/models/silero_vad.onnx")
    }

    /// Pedir GPU num host sem EP de GPU (este container; Linux sem feature) devolve sessão de CPU
    /// que roda, e a leitura diz CPU — não "GPU", não erro.
    #[test]
    fn a_gpu_plan_on_a_host_without_gpu_providers_yields_a_working_cpu_session() {
        if gpu_available() {
            return; // Máquina com GPU: o caso de queda é o teste ignorado abaixo.
        }
        let plan = Plan { device: Device::Gpu, reason: Reason::UserGpu };
        let (session, report) = build(&silero(), plan, Some(1)).expect("queda para CPU nunca é erro");
        assert!(!session.inputs().is_empty());
        assert_eq!((report.provider, report.requested, report.error), ("CPU", Device::Gpu, None));
    }

    /// O encoder int4 do Nemotron (MatMulNBits) registra o DirectML e estoura só no commit
    /// (E_INVALIDARG). Pedir GPU para ele tem de devolver sessão de CPU, não erro — e a leitura
    /// tem de dizer que a GPU falhou.
    ///   MIKA_NEMOTRON_DIR=%APPDATA%/com.mika.desktop cargo test --lib inference::session -- --ignored
    #[test]
    #[ignore = "precisa do encoder do Nemotron baixado pelo app (~790 MB) e de uma GPU DirectML"]
    fn gpu_request_falls_back_when_the_commit_rejects_an_operator() {
        let dir = std::env::var("MIKA_NEMOTRON_DIR").expect("defina MIKA_NEMOTRON_DIR");
        let path = Path::new(&dir).join("nemotron-cache/encoder.onnx");
        let plan = Plan { device: Device::Gpu, reason: Reason::UserGpu };
        let (session, report) = build(&path, plan, Some(1)).expect("GPU sem suporte tem de cair para CPU");
        assert!(!session.inputs().is_empty(), "sessão sem entradas");
        assert_eq!((report.provider, report.reason), ("CPU", Reason::GpuFailed));
        assert!(report.error.is_some());
    }
}
