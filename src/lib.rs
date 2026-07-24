#![recursion_limit = "256"]

pub mod aviation;
pub mod capabilities;
pub mod convert;
pub mod engine;
pub mod formula;
mod http;
pub mod intent;
pub mod live;
pub mod math;
pub mod mcp;
pub mod medication;
pub mod model;
pub mod package;
pub mod permission;
pub mod place;
pub mod policy;
pub mod privacy;
pub mod reference;
pub mod report;
pub mod safe_dir;
pub mod sources;
pub mod study;
pub mod study_local;
pub mod timeline;

pub use engine::{EngineConfig, ResearchEngine};
pub use model::{Facet, ResearchReport, ResearchRequest};
