//! Native provider adapters currently ported from the registry-search pilot.
mod brave;
mod community;
mod crates_io;
mod github;
mod hackernews;
mod huggingface;
mod npm;
mod reddit;
pub mod source_json;
mod stackexchange;
mod wikipedia;

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

pub use brave::BraveConfig;
pub use reddit::RedditConfig;
pub use wikipedia::WikipediaConfig;

#[derive(Clone, Debug, Default)]
pub struct BroadApiConfig {
    pub brave: BraveConfig,
    pub wikipedia: WikipediaConfig,
    pub reddit: RedditConfig,
}

/// Register the three broad API adapters owned by A04. Kept separate from
/// `register_pilots` so A01's accepted factory contract remains unchanged.
pub fn register_broad_api(
    registry: &mut Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: BroadApiConfig,
) -> Result<(), RegistryError> {
    let engine_http = http.clone();
    let engine_admission = admission.clone();
    let engine_config = config.brave;
    registry.register(
        "brave",
        Arc::new(move || {
            Box::new(brave::BraveAdapter::new(
                engine_http.clone(),
                engine_admission.clone(),
                engine_config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    let engine_http = http.clone();
    let engine_admission = admission.clone();
    let engine_config = config.wikipedia;
    registry.register(
        "wikipedia",
        Arc::new(move || {
            Box::new(wikipedia::WikipediaAdapter::new(
                engine_http.clone(),
                engine_admission.clone(),
                engine_config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    let engine_http = http;
    let engine_admission = admission;
    let engine_config = config.reddit;
    registry.register(
        "reddit",
        Arc::new(move || {
            Box::new(reddit::RedditAdapter::new(
                engine_http.clone(),
                engine_admission.clone(),
                engine_config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )?;
    Ok(())
}

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

// Python semantics shared only by the A04 adapters. Keep wire encoding separate
// from URL path handling and apply whitespace conversion only at source calls.
fn broad_query(params: &[(&str, &str)]) -> String {
    fn append(value: &str, out: &mut String) {
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        for b in value.bytes() {
            if b.is_ascii_alphanumeric() || b"_.-~".contains(&b) {
                out.push(char::from(b));
            } else if b == b' ' {
                out.push('+');
            } else {
                out.push('%');
                out.push(char::from(HEX[(b >> 4) as usize]));
                out.push(char::from(HEX[(b & 15) as usize]));
            }
        }
    }
    let mut out = String::new();
    for (index, (key, value)) in params.iter().enumerate() {
        if index != 0 {
            out.push('&');
        }
        append(key, &mut out);
        out.push('=');
        append(value, &mut out);
    }
    out
}
fn broad_whitespace(ch: char) -> bool {
    ch.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&ch)
}
fn broad_truthy(value: Option<&source_json::Value>) -> bool {
    match value {
        None | Some(source_json::Value::Null) => false,
        Some(source_json::Value::Bool(v)) => *v,
        Some(source_json::Value::Number(n)) => {
            if n.is_f64() {
                n.as_f64().is_some_and(|n| n != 0.0)
            } else {
                n.integer_text() != Some("0")
            }
        }
        Some(source_json::Value::String(v)) => !v.is_empty(),
        Some(source_json::Value::Array(v)) => !v.is_empty(),
        Some(source_json::Value::Object(v)) => !v.is_empty(),
    }
}
fn broad_type(value: &source_json::Value) -> &'static str {
    match value {
        source_json::Value::Null => "NoneType",
        source_json::Value::Bool(_) => "bool",
        source_json::Value::Number(n) => {
            if n.is_f64() {
                "float"
            } else {
                "int"
            }
        }
        source_json::Value::String(_) => "str",
        source_json::Value::Array(_) => "list",
        source_json::Value::Object(_) => "dict",
    }
}

fn broad_field_error(
    value: Option<&source_json::Value>,
    optional: bool,
    engine: &str,
    field: &str,
) -> Option<String> {
    match value {
        None => None,
        Some(source_json::Value::String(text)) => text
            .as_str()
            .is_none()
            .then(|| format!("{engine} result {field} must contain only Unicode scalar values")),
        Some(source_json::Value::Null) if optional => None,
        _ => Some(format!(
            "{engine} result {field} must be a string{}",
            if optional { " or null" } else { "" }
        )),
    }
}
fn broad_record_refusal(refusal: &mut Option<String>, error: Option<String>) {
    if refusal.is_none() {
        *refusal = error;
    }
}

fn broad_scalar_output(
    text: &source_json::Text,
    engine: &str,
    field: &str,
    refusal: &mut Option<String>,
) -> String {
    match text.scalar() {
        Some(s) => s,
        None => {
            broad_record_refusal(
                refusal,
                Some(format!(
                    "{engine} result {field} must contain only Unicode scalar values"
                )),
            );
            String::new()
        }
    }
}

// A07 native biomedical implementations; selection remains resolved policy.
pub mod clinicaltrials;
pub mod europepmc;
pub mod pubchem;
pub mod pubmed;
pub mod uniprot;
// Accepted budgeted payload component, consumed by the A07 runtime candidate.
pub mod source_payload;

pub mod source_repr;

/// Source-compatible settings for the five biomedical factories. Availability
/// does not grant selection: callers retain the resolved enabled policy.
#[derive(Clone, Debug)]
pub struct BiomedicalSettings {
    pub clinicaltrials: clinicaltrials::BiomedicalConfig,
    pub europepmc: clinicaltrials::BiomedicalConfig,
    pub pubmed: clinicaltrials::BiomedicalConfig,
    pub pubchem: clinicaltrials::BiomedicalConfig,
    pub uniprot: clinicaltrials::BiomedicalConfig,
}
impl BiomedicalSettings {
    /// Apply the source service's base-url normalization before adapter defaults.
    /// Other scalars retain F06's exact resolved variants.
    pub fn from_resolved(config: &ssx_config::Config) -> Self {
        fn resolve(
            config: &ssx_config::Config,
            name: &str,
            endpoint: &str,
        ) -> clinicaltrials::BiomedicalConfig {
            let Some(entry) = config.engines.get(name) else {
                return clinicaltrials::BiomedicalConfig::defaults(endpoint);
            };
            let mut settings = clinicaltrials::BiomedicalConfig::from_resolved(entry);
            if settings.base_url.chars().all(ssx_json::python_whitespace) {
                settings.base_url = endpoint.into();
            }
            settings
        }
        Self {
            clinicaltrials: resolve(
                config,
                "clinicaltrials",
                "https://clinicaltrials.gov/api/v2/studies",
            ),
            europepmc: resolve(
                config,
                "europepmc",
                "https://www.ebi.ac.uk/europepmc/webservices/rest",
            ),
            pubmed: resolve(
                config,
                "pubmed",
                "https://eutils.ncbi.nlm.nih.gov/entrez/eutils",
            ),
            pubchem: resolve(
                config,
                "pubchem",
                "https://pubchem.ncbi.nlm.nih.gov/rest/pug",
            ),
            uniprot: resolve(
                config,
                "uniprot",
                "https://rest.uniprot.org/uniprotkb/search",
            ),
        }
    }
}
/// Attach actual factories without changing historical F06 catalog declarations
/// or the caller's enabled/active policy. Shared handles belong to the caller.
pub fn register_biomedical(
    registry: &mut Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: BiomedicalSettings,
) -> Result<(), RegistryError> {
    macro_rules! attach {
        ($module:ident, $adapter:ident) => {{
            let http = http.clone();
            let admission = admission.clone();
            let settings = config.$module;
            registry.register(
                stringify!($module),
                Arc::new(move || {
                    Box::new($module::$adapter::new(
                        http.clone(),
                        admission.clone(),
                        settings.clone(),
                    )) as Box<dyn ssx_registry::Engine>
                }) as EngineFactory,
            )?;
        }};
    }
    attach!(clinicaltrials, ClinicalTrialsAdapter);
    attach!(europepmc, EuropePMCAdapter);
    attach!(pubmed, PubMedAdapter);
    attach!(pubchem, PubChemAdapter);
    attach!(uniprot, UniProtAdapter);
    Ok(())
}

// A08 regulatory, financial and legal sources; independent factory attachment.
pub mod edgar;
mod fda_case;
pub mod fred;
pub mod openfda;
pub mod oyez;
mod regulatory;
/// Resolved A08 settings. The composition root snapshots credentials; adapters
/// never reread process environment or change shared selection policy.
#[derive(Clone, Debug)]
pub struct RegulatorySettings {
    pub edgar: edgar::EdgarConfig,
    pub fred: fred::FredConfig,
    pub openfda: openfda::OpenFdaConfig,
    pub oyez: oyez::OyezConfig,
}
impl RegulatorySettings {
    pub fn from_resolved(config: &ssx_config::Config) -> Self {
        fn resolve(
            config: &ssx_config::Config,
            name: &str,
            endpoint: &str,
        ) -> clinicaltrials::BiomedicalConfig {
            let mut settings = config
                .engines
                .get(name)
                .map(clinicaltrials::BiomedicalConfig::from_resolved)
                .unwrap_or_else(|| clinicaltrials::BiomedicalConfig::defaults(endpoint));
            if settings.base_url.chars().all(ssx_json::python_whitespace) {
                settings.base_url = endpoint.into();
            }
            settings
        }
        Self {
            edgar: resolve(config, "edgar", edgar::ENDPOINT),
            fred: fred::FredConfig {
                settings: resolve(config, "fred", fred::ENDPOINT),
                api_key: config
                    .engines
                    .get("fred")
                    .map(|c| c.api_key.clone())
                    .unwrap_or_default(),
            },
            openfda: resolve(config, "openfda", openfda::ENDPOINT),
            oyez: oyez::OyezConfig {
                settings: resolve(config, "oyez", ""),
                search_url: None,
            },
        }
    }
}
pub fn register_regulatory(
    registry: &mut Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: RegulatorySettings,
) -> Result<(), RegistryError> {
    macro_rules! attach {
        ($module:ident, $adapter:ident) => {{
            let http = http.clone();
            let admission = admission.clone();
            let settings = config.$module;
            registry.register(
                stringify!($module),
                Arc::new(move || {
                    Box::new($module::$adapter::new(
                        http.clone(),
                        admission.clone(),
                        settings.clone(),
                    )) as Box<dyn ssx_registry::Engine>
                }) as EngineFactory,
            )?;
        }};
    }
    attach!(edgar, EdgarAdapter);
    attach!(fred, FredAdapter);
    attach!(openfda, OpenFdaAdapter);
    attach!(oyez, OyezAdapter);
    Ok(())
}
