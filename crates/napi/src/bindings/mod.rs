//! Node-API surface for running the Stillwind compiler inside Vite.

use std::collections::{BTreeMap, BTreeSet};

use napi::{Error, Result};
use napi_derive::napi;

use stillwind_core::{CompileResult, Compiler, CompilerOptions};
use stillwind_core::{
    DesignSystemSnapshot, Fingerprint, ModuleCompiler, OxcSourceAnalyzer, SourceAnalyzer, TypeFacts,
};
use stillwind_semantic::{AssetMetadata, IdentityId};
use stillwind_source::{FileId, SourceFile};

mod api;
pub use api::*;
use api::{binding_output, parse_options};
mod output;
use output::*;
mod session;
pub use session::NativeSessionError;
use session::{ProjectBindingOutput, ProjectModuleOutput, ProjectSource};
