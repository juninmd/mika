//! Aceleração de hardware dos modelos ONNX — QUEM vai para a GPU e POR QUÊ. A decisão é pura
//! ([`decide`]); [`super::session`] é quem monta a sessão e devolve o que de fato aconteceu, que
//! fica aqui ([`record`]) para a leitura ao vivo da aba Resumo (`power:profile` → `acceleration`).
//!
//! **Política por carga, não por modelo solto** ([`Workload`]):
//! - `Tiny` (Silero VAD, janela de 32 ms num LSTM minúsculo) e `PerToken` (decoder do Whisper,
//!   autoregressivo: uma cópia CPU↔GPU por token, nenhum ganho medido) ficam SEMPRE na CPU — nem a
//!   escolha "GPU" muda isso, o despacho custa mais que o cálculo.
//! - `CpuMeasured` (Nemotron): medido numa RTX 4060 Ti com DirectML — o encoder int4 nem inicializa
//!   e decoder/joiner ficam mais lentos. Automático = CPU; "GPU" explícito tenta, com queda.
//! - `Interactive` (encoder do Whisper 453 → 89 ms, Supertonic): a pessoa está esperando a
//!   resposta. GPU em TODO estado do governador, inclusive jogando: a rajada na GPU é curta (dezenas
//!   de ms por fala) e a alternativa — o mesmo cálculo na CPU com as threads já cortadas pela metade
//!   — segura núcleos que o jogo usa por ~1 s e ainda atrasa a fala. Trocar de dispositivo ao abrir
//!   o jogo também recarregaria o modelo no pior momento.
//! - `Background` (e5, busca de memória e guarda de repetição: ninguém espera por ele) cede a GPU:
//!   jogando ela é do jogo, na bateria acordar a GPU dedicada gasta mais que alguns ms de CPU, e o
//!   perfil Economia pede o mínimo. Fora disso, GPU.
//!
//! O perfil do governador `Quality` não muda nada aqui além do `Background`: ele já libera tudo.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;

use serde_json::{json, Value};

use super::session::Device;
use crate::modules::system::governor::policy::{PowerState, Profile};

/// `app.inferenceAcceleration` — Automática / GPU / CPU (aba Resumo, `PerformanceProfileCard`).
pub const CONFIG_PATH: &str = "app.inferenceAcceleration";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preference {
    Auto,
    Gpu,
    Cpu,
}

const PREFERENCES: [Preference; 3] = [Preference::Auto, Preference::Gpu, Preference::Cpu];

impl Preference {
    /// Valor desconhecido (config de outra versão) vira `Auto`, nunca erro.
    pub fn parse(raw: &str) -> Self {
        match raw {
            "gpu" => Self::Gpu,
            "cpu" => Self::Cpu,
            _ => Self::Auto,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Gpu => "gpu",
            Self::Cpu => "cpu",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Workload {
    Tiny,
    PerToken,
    CpuMeasured,
    Interactive,
    Background,
}

/// Cada sessão ONNX do app. Ordem = ordem da leitura na tela.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Model {
    WhisperEncoder,
    WhisperDecoder,
    Nemotron,
    Supertonic,
    Silero,
    E5,
}

impl Model {
    /// Espelha `inferenceModelIdSchema` (`packages/sdk/src/contracts/desktop-ipc-core-schemas.ts`).
    pub fn id(self) -> &'static str {
        match self {
            Self::WhisperEncoder => "whisperEncoder",
            Self::WhisperDecoder => "whisperDecoder",
            Self::Nemotron => "nemotron",
            Self::Supertonic => "supertonic",
            Self::Silero => "silero",
            Self::E5 => "e5",
        }
    }

    pub fn workload(self) -> Workload {
        match self {
            Self::Silero => Workload::Tiny,
            Self::WhisperDecoder => Workload::PerToken,
            Self::Nemotron => Workload::CpuMeasured,
            Self::WhisperEncoder | Self::Supertonic => Workload::Interactive,
            Self::E5 => Workload::Background,
        }
    }
}

/// Por que o dispositivo é esse. Espelha `inferenceReasonSchema` (TS) e as chaves de i18n
/// `settings.performance.accel.reason.*`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    UserCpu,
    TinyModel,
    PerToken,
    NoGpu,
    UserGpu,
    MeasuredCpu,
    Economy,
    Gaming,
    Battery,
    Accelerated,
    /// Pediu GPU, todo EP de GPU falhou (registro ou commit) e a sessão caiu para CPU.
    GpuFailed,
}

impl Reason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UserCpu => "userCpu",
            Self::TinyModel => "tinyModel",
            Self::PerToken => "perToken",
            Self::NoGpu => "noGpu",
            Self::UserGpu => "userGpu",
            Self::MeasuredCpu => "measuredCpu",
            Self::Economy => "economy",
            Self::Gaming => "gaming",
            Self::Battery => "battery",
            Self::Accelerated => "accelerated",
            Self::GpuFailed => "gpuFailed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub device: Device,
    pub reason: Reason,
}

const fn cpu(reason: Reason) -> Plan {
    Plan { device: Device::Cpu, reason }
}

const fn gpu(reason: Reason) -> Plan {
    Plan { device: Device::Gpu, reason }
}

/// A política inteira (ver o cabeçalho). `gpu_available`: este build do ORT tem algum EP de GPU
/// que o SO aceitou (DirectML/CoreML/CUDA/TensorRT/WebGPU).
pub fn decide(pref: Preference, workload: Workload, profile: Profile, state: PowerState, gpu_available: bool) -> Plan {
    match (pref, workload) {
        (Preference::Cpu, _) => cpu(Reason::UserCpu),
        (_, Workload::Tiny) => cpu(Reason::TinyModel),
        (_, Workload::PerToken) => cpu(Reason::PerToken),
        _ if !gpu_available => cpu(Reason::NoGpu),
        (Preference::Gpu, _) => gpu(Reason::UserGpu),
        (_, Workload::CpuMeasured) => cpu(Reason::MeasuredCpu),
        (_, Workload::Interactive) => gpu(Reason::Accelerated),
        (_, Workload::Background) => background(profile, state),
    }
}

fn background(profile: Profile, state: PowerState) -> Plan {
    match (profile, state) {
        (Profile::Quality, _) => gpu(Reason::Accelerated),
        (Profile::Economy, _) => cpu(Reason::Economy),
        (_, PowerState::Gaming) => cpu(Reason::Gaming),
        (_, PowerState::Battery) => cpu(Reason::Battery),
        _ => gpu(Reason::Accelerated),
    }
}

// --- Estado de processo (mesmo corte do governador: laços de vários módulos leem sem lock) ---------

static PREFERENCE: AtomicU8 = AtomicU8::new(0);

pub fn preference() -> Preference {
    PREFERENCES[PREFERENCE.load(Ordering::Relaxed) as usize % PREFERENCES.len()]
}

/// Chamado pelo governador ao subir e a cada mudança de [`CONFIG_PATH`]. Sessões já carregadas
/// com outro plano são recarregadas no próximo uso (cada cache compara o [`Plan::device`]).
pub fn set_preference(pref: Preference) {
    let index = PREFERENCES.iter().position(|p| *p == pref).unwrap_or(0);
    PREFERENCE.store(index as u8, Ordering::Relaxed);
}

/// O plano de `model` AGORA — preferência, perfil e estado do governador, EPs deste build.
pub fn plan(model: Model) -> Plan {
    use crate::modules::system::governor;
    decide(preference(), model.workload(), governor::profile(), governor::state(), super::session::gpu_available())
}

/// O que a última carga de um modelo de fato conseguiu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub requested: Device,
    /// EP efetivo: "DirectML", "CoreML", "CUDA", "TensorRT", "WebGPU" ou "CPU".
    pub provider: &'static str,
    pub reason: Reason,
    /// Erro do EP de GPU quando caiu para CPU.
    pub error: Option<String>,
}

fn reports() -> &'static Mutex<BTreeMap<Model, Report>> {
    static REPORTS: Mutex<BTreeMap<Model, Report>> = Mutex::new(BTreeMap::new());
    &REPORTS
}

pub(super) fn record(model: Model, report: Report) {
    let changed = {
        let mut map = reports().lock().unwrap_or_else(|e| e.into_inner());
        map.insert(model, report.clone()) != Some(report)
    };
    if changed {
        crate::modules::system::governor::nudge();
    }
}

/// Campo `acceleration` do payload do governador (`power:profile` / `mika:get-power-state`).
pub fn snapshot() -> Value {
    let map = reports().lock().unwrap_or_else(|e| e.into_inner());
    let models: Vec<Value> = map
        .iter()
        .map(|(model, r)| {
            json!({
                "id": model.id(),
                "requested": if r.requested == Device::Gpu { "gpu" } else { "cpu" },
                "provider": r.provider,
                "reason": r.reason.as_str(),
                "error": r.error,
            })
        })
        .collect();
    json!({
        "preference": preference().as_str(),
        "gpuProvider": super::session::gpu_provider_name(),
        "models": models,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORKLOADS: [Workload; 5] =
        [Workload::Tiny, Workload::PerToken, Workload::CpuMeasured, Workload::Interactive, Workload::Background];
    const STATES: [PowerState; 4] = [PowerState::Active, PowerState::Idle, PowerState::Gaming, PowerState::Battery];
    const PROFILES: [Profile; 3] = [Profile::Auto, Profile::Economy, Profile::Quality];

    fn auto(workload: Workload, profile: Profile, state: PowerState) -> Plan {
        decide(Preference::Auto, workload, profile, state, true)
    }

    fn every_case() -> impl Iterator<Item = (Preference, Workload, Profile, PowerState, bool)> {
        PREFERENCES.into_iter().flat_map(|pref| {
            WORKLOADS.into_iter().flat_map(move |w| {
                PROFILES.into_iter().flat_map(move |p| STATES.into_iter().flat_map(move |s| [(pref, w, p, s, true), (pref, w, p, s, false)]))
            })
        })
    }

    /// "CPU" na tela é promessa: nenhuma combinação de estado/perfil/EP pode ignorá-la.
    #[test]
    fn cpu_preference_is_absolute() {
        for (_, w, p, s, g) in every_case() {
            assert_eq!(decide(Preference::Cpu, w, p, s, g), cpu(Reason::UserCpu), "{w:?} {p:?} {s:?} {g}");
        }
    }

    /// Sem EP de GPU (Linux padrão, container, driver sem D3D12) nada pede GPU — pedir faria cada
    /// carga tentar e falhar antes de cair para CPU.
    #[test]
    fn nothing_requests_the_gpu_when_the_build_has_no_gpu_provider() {
        for (pref, w, p, s, _) in every_case() {
            assert_eq!(decide(pref, w, p, s, false).device, Device::Cpu, "{pref:?} {w:?} {p:?} {s:?}");
        }
    }

    /// Silero e o decoder do Whisper: o despacho custa mais que o cálculo — nem "GPU" os move.
    #[test]
    fn tiny_and_per_token_models_stay_on_cpu_even_when_gpu_is_forced() {
        for (p, s) in PROFILES.into_iter().flat_map(|p| STATES.into_iter().map(move |s| (p, s))) {
            assert_eq!(decide(Preference::Gpu, Workload::Tiny, p, s, true), cpu(Reason::TinyModel));
            assert_eq!(decide(Preference::Gpu, Workload::PerToken, p, s, true), cpu(Reason::PerToken));
        }
    }

    #[test]
    fn forcing_gpu_moves_everything_else_there_even_while_gaming() {
        for w in [Workload::CpuMeasured, Workload::Interactive, Workload::Background] {
            assert_eq!(decide(Preference::Gpu, w, Profile::Economy, PowerState::Gaming, true), gpu(Reason::UserGpu), "{w:?}");
        }
    }

    /// Whisper/Supertonic: quem espera é a pessoa. Nem o jogo nem a bateria os tiram da GPU.
    #[test]
    fn interactive_models_use_the_gpu_in_every_power_state() {
        for (p, s) in PROFILES.into_iter().flat_map(|p| STATES.into_iter().map(move |s| (p, s))) {
            assert_eq!(auto(Workload::Interactive, p, s), gpu(Reason::Accelerated), "{p:?} {s:?}");
        }
    }

    #[test]
    fn nemotron_stays_on_cpu_in_auto() {
        assert_eq!(auto(Workload::CpuMeasured, Profile::Quality, PowerState::Active), cpu(Reason::MeasuredCpu));
    }

    /// O pedido do governador: jogando, trabalho de fundo deixa a GPU para o jogo.
    #[test]
    fn background_work_yields_the_gpu_to_a_game_the_battery_and_economy() {
        assert_eq!(auto(Workload::Background, Profile::Auto, PowerState::Active), gpu(Reason::Accelerated));
        assert_eq!(auto(Workload::Background, Profile::Auto, PowerState::Idle), gpu(Reason::Accelerated));
        assert_eq!(auto(Workload::Background, Profile::Auto, PowerState::Gaming), cpu(Reason::Gaming));
        assert_eq!(auto(Workload::Background, Profile::Auto, PowerState::Battery), cpu(Reason::Battery));
        for s in STATES {
            assert_eq!(auto(Workload::Background, Profile::Economy, s), cpu(Reason::Economy), "{s:?}");
            assert_eq!(auto(Workload::Background, Profile::Quality, s), gpu(Reason::Accelerated), "{s:?}");
        }
    }

    #[test]
    fn unknown_preferences_fall_back_to_auto() {
        for p in PREFERENCES {
            assert_eq!(Preference::parse(p.as_str()), p);
        }
        assert_eq!(Preference::parse("turbo"), Preference::Auto);
    }

    /// Os ids e motivos viajam para o renderer como enum Zod; os dois lados têm de concordar.
    #[test]
    fn ids_and_reasons_exist_in_the_typescript_contract() {
        let contract = include_str!("../../../../../../packages/sdk/src/contracts/desktop-ipc-core-schemas.ts");
        let models = [Model::WhisperEncoder, Model::WhisperDecoder, Model::Nemotron, Model::Supertonic, Model::Silero, Model::E5];
        let reasons = [
            Reason::UserCpu,
            Reason::TinyModel,
            Reason::PerToken,
            Reason::NoGpu,
            Reason::UserGpu,
            Reason::MeasuredCpu,
            Reason::Economy,
            Reason::Gaming,
            Reason::Battery,
            Reason::Accelerated,
            Reason::GpuFailed,
        ];
        for id in models.map(Model::id).into_iter().chain(reasons.map(Reason::as_str)) {
            assert!(contract.contains(&format!("'{id}'")), "'{id}' ausente do contrato TS");
        }
    }
}
