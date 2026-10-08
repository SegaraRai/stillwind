//! Host transport for editable documents and prepared parser buffers.
//!
//! [`HostPreparedSource::input`] converts direct script documents or projected
//! component documents into analyzer input. Projection realms and editable regions
//! arrive as UTF-16 offsets in the original document and become byte spans through
//! [`stillwind_source::CoordinateIndex`]. Surrogate interiors are rejected rather than rounded. Core
//! constructors validate source formats and projection metadata. The host supplies
//! source maps and converter records.
//!
//! `document` owns editable input transport. `prepared` registers analyzer inputs.
//! `projection` validates projection transport and coordinates. `failure` resolves
//! known adapter failures against registered original sources.

mod document;
mod failure;
mod prepared;
mod projection;

pub use document::{
    HostSourceDocument, NativeFramework, NativeScriptLanguage, NativeScriptModuleKind,
    NativeScriptSyntax, NativeSourceDocument,
};
pub use failure::NativeSourceFailure;
pub use prepared::{HostPreparedSource, NativePreparedSource};
pub use projection::{HostSourceProjection, NativeConverterIdentity, NativeSourceProjection};

use projection::projection_span;
