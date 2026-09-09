//! Caller-driven CSS loading. Source identities are deliberately opaque.
use std::{collections::BTreeMap, sync::Arc};

use cssparser::{Parser, ParserInput, Token};
use stillwind_source::{FileId, SourceFile};

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
    NeedFile(CssFileRequest),
    Complete(Result<DesignSystemSnapshot, DesignSystemLoadError>),
}

#[derive(Clone, Debug)]
struct Import {
    start: usize,
    end: usize,
    specifier: String,
}

struct Frame {
    name: CssSourceName,
    imports: Vec<Import>,
    next: usize,
    cursor: usize,
    output: String,
}

/// One outstanding request; dropping this value cancels the load.
pub struct DesignSystemLoader {
    options: DesignSystemOptions,
    graph: CssSourceGraph,
    known: BTreeMap<CssSourceName, usize>,
    expanded: BTreeMap<CssSourceName, String>,
    stack: Vec<Frame>,
    pending: Option<CssFileRequest>,
    terminal: bool,
}

impl DesignSystemLoader {
    pub fn new(options: DesignSystemOptions) -> Self {
        Self {
            options,
            graph: CssSourceGraph::default(),
            known: BTreeMap::new(),
            expanded: BTreeMap::new(),
            stack: Vec::new(),
            pending: None,
            terminal: false,
        }
    }

    pub fn provide(&mut self, name: CssSourceName, content: Arc<str>) -> DesignSystemLoadStep {
        if self.terminal {
            return DesignSystemLoadStep::Complete(Err(DesignSystemLoadError::Protocol {
                message: "provide called after completion".into(),
            }));
        }
        match self.accept(name, content) {
            Ok(step) => step,
            Err(error) => {
                self.terminal = true;
                DesignSystemLoadStep::Complete(Err(error))
            }
        }
    }

    fn accept(
        &mut self,
        name: CssSourceName,
        content: Arc<str>,
    ) -> Result<DesignSystemLoadStep, DesignSystemLoadError> {
        let request = self.pending.take();
        if let Some(&index) = self.known.get(&name) {
            if self.graph.sources[index].source.text() != content.as_ref() {
                return Err(DesignSystemLoadError::Protocol {
                    message: format!("conflicting content for {}", name.0),
                });
            }
            if self.stack.iter().any(|frame| frame.name == name) {
                let mut chain: Vec<_> = self
                    .stack
                    .iter()
                    .filter_map(|frame| {
                        frame.next.checked_sub(1).map(|index| CssFileRequest {
                            importer: frame.name.clone(),
                            specifier: frame.imports[index].specifier.clone(),
                            kind: CssDependencyKind::Import,
                        })
                    })
                    .collect();
                if chain.is_empty() {
                    chain.extend(request.clone());
                }
                return Err(DesignSystemLoadError::Cycle { chain });
            }
            if let Some(parent) = self.stack.last_mut() {
                parent.output.push_str(&self.expanded[&name]);
            }
        } else {
            let imports =
                imports(&content).map_err(|message| DesignSystemLoadError::InvalidCss {
                    source: name.clone(),
                    message,
                })?;
            let id = u32::try_from(self.graph.sources.len() + 1).map_err(|_| {
                DesignSystemLoadError::Protocol {
                    message: "too many CSS sources".into(),
                }
            })?;
            let source = Arc::new(SourceFile::new(FileId(id), name.0.clone(), content));
            // Validate each supplied source separately so diagnostics retain its identity.
            stillwind_css::parse_tailwind_theme(&source).map_err(|error| {
                DesignSystemLoadError::InvalidCss {
                    source: name.clone(),
                    message: error.to_string(),
                }
            })?;
            self.known.insert(name.clone(), self.graph.sources.len());
            self.graph.sources.push(CssSource {
                name: name.clone(),
                source,
            });
            self.stack.push(Frame {
                name: name.clone(),
                imports,
                next: 0,
                cursor: 0,
                output: String::new(),
            });
        }
        if let Some(request) = request {
            self.graph.edges.push(CssSourceEdge {
                request,
                target: name,
            });
        }
        self.advance()
    }

    fn advance(&mut self) -> Result<DesignSystemLoadStep, DesignSystemLoadError> {
        while let Some(frame) = self.stack.last_mut() {
            let source = &self.graph.sources[self.known[&frame.name]].source;
            if let Some(import) = frame.imports.get(frame.next) {
                frame
                    .output
                    .push_str(&source.text()[frame.cursor..import.start]);
                frame.cursor = import.end;
                frame.next += 1;
                let request = CssFileRequest {
                    importer: frame.name.clone(),
                    specifier: import.specifier.clone(),
                    kind: CssDependencyKind::Import,
                };
                self.pending = Some(request.clone());
                return Ok(DesignSystemLoadStep::NeedFile(request));
            }
            frame.output.push_str(&source.text()[frame.cursor..]);
            let frame = self.stack.pop().expect("active frame");
            if let Some(parent) = self.stack.last_mut() {
                parent.output.push_str(&frame.output);
                self.expanded.insert(frame.name, frame.output);
            } else {
                let source = SourceFile::new(FileId(0), frame.name.0.clone(), frame.output);
                let theme = stillwind_css::parse_tailwind_theme(&source).map_err(|error| {
                    DesignSystemLoadError::InvalidCss {
                        source: frame.name.clone(),
                        message: error.to_string(),
                    }
                })?;
                let snapshot = DesignSystemSnapshot::from_loaded_theme(
                    theme,
                    self.options.clone(),
                    std::mem::take(&mut self.graph),
                )
                .map_err(|error| DesignSystemLoadError::InvalidCss {
                    source: frame.name,
                    message: error.to_string(),
                })?;
                self.terminal = true;
                return Ok(DesignSystemLoadStep::Complete(Ok(snapshot)));
            }
        }
        unreachable!("initial source always creates a frame")
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
        if !matches!(token, Token::AtKeyword(name) if name.eq_ignore_ascii_case("import")) {
            continue;
        }
        let specifier = parser
            .expect_url_or_string()
            .map_err(|error| format!("invalid @import: {error:?}"))?
            .to_string();
        let tail_start = parser.position();
        let mut tail_end = tail_start;
        loop {
            let position = parser.position();
            match parser.next_including_whitespace_and_comments() {
                Ok(Token::Semicolon) => {
                    tail_end = position;
                    break;
                }
                Ok(_) => (),
                Err(_) => break,
            }
        }
        if is_framework(&specifier) {
            continue;
        }
        if !parser.slice(tail_start..tail_end).trim().is_empty() {
            return Err("conditional/layered external @import is not supported for design-system definitions".into());
        }
        imports.push(Import {
            start,
            end: parser.position().byte_index(),
            specifier,
        });
    }
    Ok(imports)
}
