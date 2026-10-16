//! Native provider adapters currently ported from the registry-search pilot.
mod crates_io;
mod npm;

use crate::http::AdapterHttp;
use ssx_application::rate::RateLimiter;
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission};
use ssx_domain::rate::{EngineKey, RateReason};
use ssx_registry::{EngineFactory, Registry, RegistryError};
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Instant;

pub use crates_io::CratesConfig;
pub use npm::NpmConfig;

/// Resolved source-compatible settings for the A01 pilot adapters.
#[derive(Clone, Debug, Default)]
pub struct PilotConfig {
    pub npm: NpmConfig,
    pub crates: CratesConfig,
}

/// Attach the two implemented F06 descriptors to their executable factories.
/// The caller owns the shared transport lifetime; adapters never close it.
pub fn register_pilots(
    registry: &mut Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: PilotConfig,
) -> Result<(), RegistryError> {
    let npm_http = http.clone();
    let npm_config = config.npm;
    let npm_admission = admission.clone();
    registry.register(
        "npm",
        Arc::new(move || {
            Box::new(npm::NpmAdapter::new(
                npm_http.clone(),
                npm_admission.clone(),
                npm_config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    let crates_http = http;
    let crates_config = config.crates;
    let crates_admission = admission;
    registry.register(
        "crates",
        Arc::new(move || {
            Box::new(crates_io::CratesAdapter::new(
                crates_http.clone(),
                crates_admission.clone(),
                crates_config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    Ok(())
}

/// Explicit no-quota mode for tests and deployments with no configured limiter.
#[derive(Default)]
pub struct AllowAllAdmission;
impl SearchAdmission for AllowAllAdmission {
    fn admit<'a>(&'a self, _: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async { AdmissionDecision::Allowed })
    }
}

/// Adapter-facing wrapper around the accepted F10 policy; quota behavior stays
/// in `RateLimiter`, while this bridge translates its classified decision.
pub struct F10Admission {
    limiter: Arc<tokio::sync::Mutex<RateLimiter>>,
}
impl F10Admission {
    /// The composition root retains this handle for lifecycle, reactivation and
    /// cooldown operations; the adapter bridge only acquires quota decisions.
    pub fn new(limiter: Arc<tokio::sync::Mutex<RateLimiter>>) -> Self {
        Self { limiter }
    }
}
impl SearchAdmission for F10Admission {
    fn admit<'a>(&'a self, engine: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async move {
            let Ok(key) = EngineKey::new(engine) else {
                return AdmissionDecision::Unavailable;
            };
            let mut limiter = self.limiter.lock().await;
            let now = f10_monotonic_seconds();
            match limiter.acquire(&key, 1, now).await {
                Ok(decision) if decision.allowed => AdmissionDecision::Allowed,
                Ok(decision) if decision.reason == RateReason::QuotaUnavailable => {
                    AdmissionDecision::Unavailable
                }
                Ok(_) => AdmissionDecision::Denied,
                Err(_) => AdmissionDecision::Unavailable,
            }
        })
    }
}

/// Shared process-local monotonic epoch for all `RateLimiter` operations used
/// with `F10Admission`. Composition roots must pass this value to limiter
/// cooldown APIs; a second `Instant` origin would invalidate stored deadlines.
pub fn f10_monotonic_seconds() -> f64 {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    ORIGIN.get_or_init(Instant::now).elapsed().as_secs_f64()
}

mod dockerhub;
mod pypi;
mod rubygems;
pub use dockerhub::DockerHubConfig;
pub use pypi::PyPIConfig;
pub use rubygems::RubyGemsConfig;

/// Source-compatible package adapter settings. Caller retains transport/admission ownership.
#[derive(Clone, Debug, Default)]
pub struct PackageConfig {
    pub pypi: PyPIConfig,
    pub rubygems: RubyGemsConfig,
    pub dockerhub: DockerHubConfig,
}

/// Register executable named-engine factories without changing catalog metadata.
pub fn register_packages(
    registry: &mut Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: PackageConfig,
) -> Result<(), RegistryError> {
    let h = http.clone();
    let a = admission.clone();
    let c = config.pypi;
    registry.register(
        "pypi",
        Arc::new(move || {
            Box::new(pypi::PyPIAdapter::new(h.clone(), a.clone(), c.clone()))
                as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    let h = http.clone();
    let a = admission.clone();
    let c = config.rubygems;
    registry.register(
        "rubygems",
        Arc::new(move || {
            Box::new(rubygems::RubyGemsAdapter::new(
                h.clone(),
                a.clone(),
                c.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    let h = http.clone();
    let a = admission.clone();
    let c = config.dockerhub;
    registry.register(
        "dockerhub",
        Arc::new(move || {
            Box::new(dockerhub::DockerHubAdapter::new(
                h.clone(),
                a.clone(),
                c.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    Ok(())
}

mod html_timeout;
mod duckduckgo;
mod google;
pub use duckduckgo::{DuckDuckGoConfig, ScrapeConfig};
pub use google::GoogleConfig;

/// Executable HTML factories; F06 catalog eligibility remains independent.
#[derive(Clone, Debug, Default)]
pub struct HtmlConfig {
    pub duckduckgo: DuckDuckGoConfig,
    pub google: GoogleConfig,
}
pub fn register_html(
    registry: &mut Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: HtmlConfig,
) -> Result<(), RegistryError> {
    let h = http.clone();
    let a = admission.clone();
    let c = config.duckduckgo;
    registry.register(
        "duckduckgo",
        Arc::new(move || {
            Box::new(duckduckgo::DuckDuckGoAdapter::new(
                h.clone(),
                a.clone(),
                c.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    let c = config.google;
    registry.register(
        "google",
        Arc::new(move || {
            Box::new(google::GoogleAdapter::new(
                http.clone(),
                admission.clone(),
                c.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    Ok(())
}

pub(crate) fn html_detector_flags(raw: &ssx_source_codecs::Text) -> crate::html::DetectorFlags {
    crate::html::DetectorFlags {
        duckduckgo_challenge: duckduckgo::challenge(raw),
        duckduckgo_legitimate_empty: duckduckgo::legitimate_empty(raw),
        google_challenge: google::challenge(raw),
    }
}

/// Closed helper-facing config. Invalid types raise only upon a selected row,
/// preserving source valid-empty behavior without copying arbitrary config.
#[derive(Clone, Copy)]
pub(crate) enum RowLimit {
    Number(f64),
    Unlimited,
    Invalid(u8),
}
impl RowLimit {
    fn from_value(value: &ssx_json::Value) -> Self {
        match value {
            ssx_json::Value::Bool(value) => Self::Number(f64::from(u8::from(*value))),
            ssx_json::Value::Number(number) => {
                number.as_f64().map(Self::Number).unwrap_or(Self::Unlimited)
            }
            ssx_json::Value::Null => Self::Invalid(0),
            ssx_json::Value::String(_) => Self::Invalid(1),
            ssx_json::Value::Array(_) => Self::Invalid(2),
            ssx_json::Value::Object(_) => Self::Invalid(3),
        }
    }
    fn reached(self, count: usize) -> Result<bool, String> {
        match self {
            Self::Number(number) => Ok(count as f64 >= number),
            Self::Unlimited => Ok(false),
            Self::Invalid(tag) => {
                let name = match tag {
                    0 => "NoneType",
                    1 => "str",
                    2 => "list",
                    3 => "dict",
                    _ => return Err("invalid HTML row-limit tag".into()),
                };
                Err(format!(
                    "'>=' not supported between instances of 'int' and '{name}'"
                ))
            }
        }
    }
}
