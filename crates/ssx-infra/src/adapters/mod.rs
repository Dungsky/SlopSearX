//! Native provider adapters currently ported from the registry-search pilot.
pub mod arxiv;
mod arxiv_accounting;
mod arxiv_xml;
mod community;
mod crates_io;
mod github;
mod hackernews;
mod huggingface;
mod npm;
mod stackexchange;

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

pub use community::CommunitySettings;
pub use github::GitHubConfig;
pub use hackernews::HackerNewsConfig;
pub use huggingface::HuggingFaceConfig;
pub use stackexchange::StackExchangeConfig;

/// A03 settings projected by the composition root; no configuration loader or
/// credential discovery is hidden in an adapter.
#[derive(Clone, Debug, Default)]
pub struct CommunityConfig {
    pub github: GitHubConfig,
    pub huggingface: HuggingFaceConfig,
    pub stackexchange: StackExchangeConfig,
    pub hackernews: HackerNewsConfig,
}
/// Attach A03 factories while retaining caller ownership of transport/admission.
pub fn register_community(
    registry: &mut Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: CommunityConfig,
) -> Result<(), RegistryError> {
    let engine_http = http.clone();
    let engine_admission = admission.clone();
    let engine_config = config.github;
    registry.register(
        "github",
        Arc::new(move || {
            Box::new(github::GitHubAdapter::new(
                engine_http.clone(),
                engine_admission.clone(),
                engine_config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    let engine_http = http.clone();
    let engine_admission = admission.clone();
    let engine_config = config.huggingface;
    registry.register(
        "huggingface",
        Arc::new(move || {
            Box::new(huggingface::HuggingFaceAdapter::new(
                engine_http.clone(),
                engine_admission.clone(),
                engine_config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    let engine_http = http.clone();
    let engine_admission = admission.clone();
    let engine_config = config.stackexchange;
    registry.register(
        "stackexchange",
        Arc::new(move || {
            Box::new(stackexchange::StackExchangeAdapter::new(
                engine_http.clone(),
                engine_admission.clone(),
                engine_config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    let engine_http = http.clone();
    let engine_admission = admission.clone();
    let engine_config = config.hackernews;
    registry.register(
        "hackernews",
        Arc::new(move || {
            Box::new(hackernews::HackerNewsAdapter::new(
                engine_http.clone(),
                engine_admission.clone(),
                engine_config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    Ok(())
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

/// Expat candidate, not a registered adapter.
pub mod arxiv_expat;

mod arxiv_expat_wire;

pub mod scholarly_wire;
mod scholarly_word;
