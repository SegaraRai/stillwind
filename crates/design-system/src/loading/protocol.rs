//! Host-facing loader protocol types: opaque source names, outstanding requests, and
//! the source graph retained by a completed snapshot.
//!
//! Nothing here has path semantics. A [`CssSourceName`] is an identity the host chose,
//! compared and cloned byte-for-byte; the loader never joins it with a specifier,
//! normalizes separators, or inspects an extension. A host does perform path resolution
//! and file reads before answering a request, but outside these types: it supplies the
//! resulting buffer under a name it chose itself.

use std::sync::Arc;

use lean_string::LeanStr;

use stillwind_source::{CssSourceName, FileId, SourceFile, SourceUri, Span};

use super::state::DesignSystemLoadError;
use crate::DesignSystemSnapshot;

/// Why the loader needs a source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CssDependencyKind {
    /// An authored `@import` in the importer's CSS.
    Import,
    /// An authored `@plugin`. The host runs the plugin that the specifier names and
    /// answers with the CSS that the plugin generates.
    Plugin {
        /// Trimmed text of the options block, or empty without a block.
        options: LeanStr,
    },
}

/// One unresolved dependency, identified by the ID the host must answer with.
///
/// The host resolves exactly the pair `(importer, specifier)`; the returned name
/// becomes the imported source's identity for cycle and duplicate detection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssFileRequest {
    /// Response ID, issued in increasing order starting at 1.
    pub id: u32,
    /// Name of the source that authored the import.
    pub importer: CssSourceName,
    /// Decoded CSS import specifier text; escapes are resolved by the tokenizer.
    pub specifier: LeanStr,
    /// Kind of dependency that produced this request.
    pub kind: CssDependencyKind,
}

/// One parsed CSS buffer together with the name it was supplied under.
#[derive(Clone, Debug)]
pub struct CssSource {
    /// Name the host supplied this buffer under.
    pub name: CssSourceName,
    /// Parsed buffer. Its file ID was allocated in acceptance order and is the identity
    /// used by every pre-expansion span for this source.
    pub source: Arc<SourceFile>,
    /// Whether stylesheet lint checks this source. It is true when any response that
    /// supplied this name set [`CssFileInput::lint`].
    pub lint: bool,
}

/// A resolved import edge. `target` is the name the host supplied for the request,
/// which may differ from the authored specifier.
#[derive(Clone, Debug)]
pub struct CssSourceEdge {
    /// The request that was answered.
    pub request: CssFileRequest,
    /// Name the host supplied as the answer.
    pub target: CssSourceName,
}

/// Every source the host supplied for one load, in the order the loader accepted them,
/// with the resolved edges and the expansion mapping produced by import expansion.
///
/// Hosts register watches and cache dependencies from this graph. It is a record of
/// host decisions, not a resolution result the loader computed.
#[derive(Clone, Debug, Default)]
pub struct CssSourceGraph {
    /// Every accepted source, in acceptance order.
    pub sources: Vec<CssSource>,
    /// Resolved import edges.
    pub edges: Vec<CssSourceEdge>,
    /// Expansion mappings from generated text back to authored buffers.
    pub mappings: Vec<CssSourceMapping>,
    /// Resource references the host must substitute.
    pub resources: Vec<CssResourceReference>,
}

/// A resource placeholder in generated CSS. Hosts resolve specifiers from
/// `source_name`, then replace the placeholder before their CSS asset pipeline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssResourceReference {
    /// Unique marker emitted instead of the authored resource reference.
    pub placeholder: LeanStr,
    /// Source that authored the reference.
    pub source_name: CssSourceName,
    /// Decoded resource specifier the host must resolve.
    pub specifier: LeanStr,
    /// Span of the authored reference in its own source buffer.
    pub span: Span,
}

/// One run of expanded text and the original source bytes it came from.
///
/// `source_end` is set only for a length-changing resource substitution, where the
/// generated text cannot be offset back into the original value.
#[derive(Clone, Debug)]
pub struct CssSourceMapping {
    /// Range of the expanded text this mapping covers.
    pub generated: std::ops::Range<usize>,
    /// Source the generated text came from.
    pub file: FileId,
    /// Byte start in that source.
    pub source_start: usize,
    /// Explicit original end for a length-changing resource substitution.
    pub source_end: Option<usize>,
}

/// The result of one loader step: either what the host must supply next, or the
/// terminal outcome.
///
/// [`DesignSystemLoadStep::NeedFile`] lists every outstanding request by increasing ID,
/// including requests that stayed unanswered from earlier steps.
/// [`DesignSystemLoadStep::Complete`] is terminal, including a failed completion; the
/// loader is not reused for a reload.
#[derive(Debug)]
pub enum DesignSystemLoadStep {
    /// The host must supply these sources next.
    NeedFile(Vec<CssFileRequest>),
    /// Terminal outcome of the load.
    Complete(Result<DesignSystemSnapshot, DesignSystemLoadError>),
}

/// A response to a request ID. The initial batch contains exactly one root source;
/// later batches answer previously issued IDs only.
#[derive(Clone, Debug)]
pub struct CssFileInput {
    /// ID being answered, which must be outstanding in this loader.
    pub id: u32,
    /// Name the host assigns to this source; repeated names must carry identical
    /// content.
    pub name: CssSourceName,
    /// Host-supplied document identity. `None` means this is an internal CSS
    /// buffer without a reportable document location.
    pub uri: Option<SourceUri>,
    /// Exact source name to write into a generated source map.
    pub source_map_source: Option<Arc<str>>,
    /// Full text of the source.
    pub content: Arc<str>,
    /// Whether stylesheet lint checks this source. Hosts set it for the user's own CSS
    /// and leave it false for package CSS, such as the `stillwind` package and files
    /// under `node_modules`. It does not change the snapshot's definitions or output.
    pub lint: bool,
}
