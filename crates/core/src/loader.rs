//! Caller-driven CSS loading. Source identities are deliberately opaque.
use std::{collections::BTreeMap, sync::Arc};

use cssparser::{Parser, ParserInput, Token};
use stillwind_source::{FileId, SourceFile, Span};

use crate::{CompilerOptions, DesignSystemSnapshot};

pub type DesignSystemOptions = CompilerOptions;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CssSourceName(pub Arc<str>);

impl From<&str> for CssSourceName {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CssDependencyKind {
    Import,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssFileRequest {
    pub id: u32,
    pub importer: CssSourceName,
    pub specifier: String,
    pub kind: CssDependencyKind,
}

#[derive(Clone, Debug)]
pub struct CssSource {
    pub name: CssSourceName,
    pub source: Arc<SourceFile>,
}

#[derive(Clone, Debug)]
pub struct CssSourceEdge {
    pub request: CssFileRequest,
    pub target: CssSourceName,
}

#[derive(Clone, Debug, Default)]
pub struct CssSourceGraph {
    pub sources: Vec<CssSource>,
    pub edges: Vec<CssSourceEdge>,
    pub mappings: Vec<CssSourceMapping>,
}

#[derive(Clone, Debug)]
pub struct CssSourceMapping {
    pub generated: std::ops::Range<usize>,
    pub file: FileId,
    pub source_start: usize,
}

#[derive(Clone, Debug, Default)]
struct ExpandedSource {
    text: String,
    mappings: Vec<CssSourceMapping>,
}

impl ExpandedSource {
    fn append_import(&mut self, other: &Self, wrappers: &[String]) {
        for wrapper in wrappers {
            self.text.push_str(wrapper);
            self.text.push('{');
        }
        self.append(other);
        for _ in wrappers {
            self.text.push('}');
        }
    }
    fn append_source(&mut self, source: &SourceFile, range: std::ops::Range<usize>) {
        let start = self.text.len();
        self.text.push_str(&source.text()[range.clone()]);
        self.mappings.push(CssSourceMapping {
            generated: start..self.text.len(),
            file: source.id(),
            source_start: range.start,
        });
    }
    fn append(&mut self, other: &Self) {
        let offset = self.text.len();
        self.text.push_str(&other.text);
        self.mappings
            .extend(other.mappings.iter().map(|mapping| CssSourceMapping {
                generated: mapping.generated.start + offset..mapping.generated.end + offset,
                file: mapping.file,
                source_start: mapping.source_start,
            }));
    }
    fn original_span(&self, span: Span) -> Option<Span> {
        let start = span.start.0 as usize;
        let end = span.end.0 as usize;
        self.mappings
            .iter()
            .find(|mapping| mapping.generated.start <= start && end <= mapping.generated.end)
            .map(|mapping| {
                let start = mapping.source_start + start - mapping.generated.start;
                Span::new(
                    mapping.file,
                    u32::try_from(start).expect("source offset"),
                    u32::try_from(start + span.len() as usize).expect("source offset"),
                )
            })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DesignSystemLoadError {
    InvalidCss {
        source: CssSourceName,
        message: String,
    },
    Protocol {
        message: String,
    },
    Cycle {
        chain: Vec<CssFileRequest>,
    },
}

impl std::fmt::Display for DesignSystemLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCss { source, message } => write!(f, "{}: {message}", source.0),
            Self::Protocol { message } => write!(f, "CSS loader protocol error: {message}"),
            Self::Cycle { chain } => write!(f, "CSS import cycle: {chain:?}"),
        }
    }
}
impl std::error::Error for DesignSystemLoadError {}

#[derive(Debug)]
pub enum DesignSystemLoadStep {
    NeedFile(Vec<CssFileRequest>),
    Complete(Result<DesignSystemSnapshot, DesignSystemLoadError>),
}

#[derive(Clone, Debug)]
struct Import {
    start: usize,
    end: usize,
    specifier: String,
    wrappers: Vec<String>,
}

/// A response to a request ID. The initial batch contains exactly one root source.
#[derive(Clone, Debug)]
pub struct CssFileInput {
    pub id: u32,
    pub name: CssSourceName,
    pub content: Arc<str>,
}

struct Node {
    imports: Vec<Import>,
    requests: Vec<CssFileRequest>,
    targets: Vec<Option<usize>>,
}

/// Caller-driven loading with independent, monotonically increasing request IDs.
/// `NeedFile` lists all outstanding requests; hosts may respond in any order.
pub struct DesignSystemLoader {
    options: DesignSystemOptions,
    graph: CssSourceGraph,
    known: BTreeMap<CssSourceName, usize>,
    nodes: Vec<Node>,
    pending: BTreeMap<u32, (usize, usize)>,
    next_id: u32,
    terminal: bool,
}

impl DesignSystemLoader {
    pub fn new(options: DesignSystemOptions) -> Self {
        Self {
            options,
            graph: CssSourceGraph::default(),
            known: BTreeMap::new(),
            nodes: Vec::new(),
            pending: BTreeMap::new(),
            next_id: 1,
            terminal: false,
        }
    }

    pub fn provide(
        &mut self,
        id: u32,
        name: CssSourceName,
        content: Arc<str>,
    ) -> DesignSystemLoadStep {
        self.provide_many([CssFileInput { id, name, content }])
    }

    pub fn provide_many(
        &mut self,
        files: impl IntoIterator<Item = CssFileInput>,
    ) -> DesignSystemLoadStep {
        if self.terminal {
            return DesignSystemLoadStep::Complete(Err(Self::protocol(
                "provide called after completion",
            )));
        }
        match self.accept_many(files.into_iter().collect()) {
            Ok(step) => step,
            Err(error) => {
                self.terminal = true;
                DesignSystemLoadStep::Complete(Err(error))
            }
        }
    }

    fn protocol(message: impl Into<String>) -> DesignSystemLoadError {
        DesignSystemLoadError::Protocol {
            message: message.into(),
        }
    }

    fn accept_many(
        &mut self,
        files: Vec<CssFileInput>,
    ) -> Result<DesignSystemLoadStep, DesignSystemLoadError> {
        let initial = self.nodes.is_empty();
        if initial && files.len() != 1 {
            return Err(Self::protocol(
                "initial provide must contain exactly one file (conventionally ID 0)",
            ));
        }
        // Validate against requests issued before this batch, including duplicates.
        let mut ids = std::collections::BTreeSet::new();
        for file in &files {
            if !ids.insert(file.id) || (!initial && !self.pending.contains_key(&file.id)) {
                return Err(Self::protocol(format!(
                    "unknown or duplicate request ID {}",
                    file.id
                )));
            }
        }
        for file in files {
            let parent = if initial {
                None
            } else {
                self.pending.remove(&file.id)
            };
            let index = self.accept_source(file.name.clone(), file.content)?;
            if let Some((source, import)) = parent {
                self.nodes[source].targets[import] = Some(index);
                self.graph.edges.push(CssSourceEdge {
                    request: self.nodes[source].requests[import].clone(),
                    target: file.name,
                });
            }
        }
        // Also detect cycles in resolved subgraphs while unrelated I/O is outstanding.
        let mut state = vec![0; self.nodes.len()];
        self.check_cycles(0, &mut state, &mut Vec::new())?;
        if !self.pending.is_empty() {
            return Ok(DesignSystemLoadStep::NeedFile(
                self.pending
                    .values()
                    .map(|&(source, import)| self.nodes[source].requests[import].clone())
                    .collect(),
            ));
        }
        let output = self.expand(0, &mut BTreeMap::new());
        let name = self.graph.sources[0].name.clone();
        let source = SourceFile::new(FileId(u32::MAX), name.0.clone(), output.text.as_str());
        let mut theme = stillwind_css::parse_tailwind_theme(&source).map_err(|error| {
            DesignSystemLoadError::InvalidCss {
                source: name.clone(),
                message: error.to_string(),
            }
        })?;
        for token in &mut theme.tokens {
            if token.name.span.file == source.id() {
                if let Some(span) = output.original_span(token.name.span) {
                    token.name.span = span;
                }
                if let Some(span) = output.original_span(token.value.span) {
                    token.value.span = span;
                }
            }
        }
        self.graph.mappings = output.mappings;
        let snapshot = DesignSystemSnapshot::from_loaded_theme(
            theme,
            self.options.clone(),
            std::mem::take(&mut self.graph),
        )
        .map_err(|error| DesignSystemLoadError::InvalidCss {
            source: name,
            message: error.to_string(),
        })?;
        self.terminal = true;
        Ok(DesignSystemLoadStep::Complete(Ok(snapshot)))
    }

    fn accept_source(
        &mut self,
        name: CssSourceName,
        content: Arc<str>,
    ) -> Result<usize, DesignSystemLoadError> {
        if let Some(&index) = self.known.get(&name) {
            if self.graph.sources[index].source.text() != content.as_ref() {
                return Err(Self::protocol(format!(
                    "conflicting content for {}",
                    name.0
                )));
            }
            return Ok(index);
        }
        let imports = imports(&content).map_err(|message| DesignSystemLoadError::InvalidCss {
            source: name.clone(),
            message,
        })?;
        let index = self.nodes.len();
        let id = u32::try_from(index + 1).map_err(|_| Self::protocol("too many CSS sources"))?;
        let source = Arc::new(SourceFile::new(FileId(id), name.0.clone(), content));
        stillwind_css::parse_tailwind_theme(&source).map_err(|error| {
            DesignSystemLoadError::InvalidCss {
                source: name.clone(),
                message: error.to_string(),
            }
        })?;
        let mut requests = Vec::new();
        for (import_index, import) in imports.iter().enumerate() {
            let id = self.next_id;
            self.next_id = id
                .checked_add(1)
                .ok_or_else(|| Self::protocol("CSS request IDs exhausted"))?;
            requests.push(CssFileRequest {
                id,
                importer: name.clone(),
                specifier: import.specifier.clone(),
                kind: CssDependencyKind::Import,
            });
            self.pending.insert(id, (index, import_index));
        }
        self.nodes.push(Node {
            targets: vec![None; imports.len()],
            imports,
            requests,
        });
        self.known.insert(name.clone(), index);
        self.graph.sources.push(CssSource { name, source });
        Ok(index)
    }

    fn check_cycles(
        &self,
        index: usize,
        state: &mut [u8],
        chain: &mut Vec<CssFileRequest>,
    ) -> Result<(), DesignSystemLoadError> {
        if state[index] == 1 {
            return Err(DesignSystemLoadError::Cycle {
                chain: chain.clone(),
            });
        }
        if state[index] == 2 {
            return Ok(());
        }
        state[index] = 1;
        for (import, target) in self.nodes[index].targets.iter().enumerate() {
            if let Some(target) = target {
                chain.push(self.nodes[index].requests[import].clone());
                self.check_cycles(*target, state, chain)?;
                chain.pop();
            }
        }
        state[index] = 2;
        Ok(())
    }

    fn expand(&self, index: usize, cache: &mut BTreeMap<usize, ExpandedSource>) -> ExpandedSource {
        if let Some(output) = cache.get(&index) {
            return output.clone();
        }
        let source = &self.graph.sources[index].source;
        let node = &self.nodes[index];
        let mut output = ExpandedSource::default();
        let mut cursor = 0;
        for (import, target) in node.imports.iter().zip(&node.targets) {
            output.append_source(source, cursor..import.start);
            output.append_import(
                &self.expand(target.expect("all requests resolved"), cache),
                &import.wrappers,
            );
            cursor = import.end;
        }
        output.append_source(source, cursor..source.text().len());
        cache.insert(index, output.clone());
        output
    }
}

fn is_framework(name: &str) -> bool {
    matches!(
        name,
        "tailwindcss"
            | "tailwindcss/theme"
            | "tailwindcss/preflight"
            | "tailwindcss/utilities"
            | "stillwind"
            | "stillwind/theme.css"
            | "stillwind/preflight.css"
            | "stillwind/utilities.css"
            | "stillwind/builtins.css"
    )
}

/// Tokenization skips comments, strings and nested blocks, and decodes CSS escapes.
fn imports(css: &str) -> Result<Vec<Import>, String> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    let mut imports = Vec::new();
    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            break;
        };
        if matches!(
            token,
            Token::CurlyBracketBlock
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::Function(_)
        ) {
            parser
                .parse_nested_block(|nested| {
                    while nested.next_including_whitespace_and_comments().is_ok() {}
                    Ok::<_, cssparser::ParseError<'_, ()>>(())
                })
                .map_err(|error| format!("invalid CSS block: {error:?}"))?;
            continue;
        }
        if !matches!(token, Token::AtKeyword(name) if name.eq_ignore_ascii_case("import")) {
            continue;
        }
        let specifier = parser
            .expect_url_or_string()
            .map_err(|error| format!("invalid @import: {error:?}"))?
            .to_string();
        let tail_start = parser.position();
        loop {
            match parser.next_including_whitespace_and_comments() {
                Ok(Token::Semicolon) => {
                    break;
                }
                Ok(_) => (),
                Err(_) => break,
            }
        }
        if is_framework(&specifier) {
            continue;
        }
        let tail = parser.slice(tail_start..parser.position());
        let wrappers = import_wrappers(tail.strip_suffix(';').unwrap_or(tail))?;
        imports.push(Import {
            start,
            end: parser.position().byte_index(),
            specifier,
            wrappers,
        });
    }
    Ok(imports)
}

fn import_wrappers(tail: &str) -> Result<Vec<String>, String> {
    let mut input = ParserInput::new(tail);
    let mut parser = Parser::new(&mut input);
    let mut wrappers = Vec::new();
    while !parser.is_exhausted() {
        let state = parser.state();
        match parser
            .next()
            .map_err(|error| format!("invalid import modifiers: {error:?}"))?
        {
            Token::Ident(name) if name.eq_ignore_ascii_case("layer") => {
                wrappers.push("@layer".into());
            }
            Token::Function(name)
                if name.eq_ignore_ascii_case("layer") || name.eq_ignore_ascii_case("supports") =>
            {
                let layer = name.eq_ignore_ascii_case("layer");
                let condition = parser
                    .parse_nested_block(|nested| {
                        let start = nested.position();
                        while nested.next_including_whitespace_and_comments().is_ok() {}
                        Ok::<_, cssparser::ParseError<'_, ()>>(
                            nested.slice_from(start).trim().to_owned(),
                        )
                    })
                    .map_err(|error| format!("invalid import modifier: {error:?}"))?;
                wrappers.push(if layer {
                    format!("@layer {condition}")
                } else {
                    format!("@supports ({condition})")
                });
            }
            _ => {
                wrappers.push(format!(
                    "@media {}",
                    tail[state.position().byte_index()..].trim()
                ));
                break;
            }
        }
    }
    Ok(wrappers)
}
