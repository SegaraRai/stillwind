use super::{CompileError, HashSet, MacroExtractor, is_strict_ancestor};

impl MacroExtractor<'_> {
    pub(super) fn validate_handles(&mut self) {
        let names = self
            .handle_uses
            .iter()
            .map(|handle_use| handle_use.name.clone())
            .collect::<HashSet<_>>();
        for name in names {
            let uses = self
                .handle_uses
                .iter()
                .filter(|handle_use| handle_use.name == name)
                .collect::<Vec<_>>();
            let is_scoped = self
                .handle_definitions
                .iter()
                .any(|definition| definition.name.value == name && definition.scope.is_some());
            let has_anchor = uses
                .iter()
                .any(|handle_use| handle_use.projection == "anchor");
            if !has_anchor {
                for handle_use in uses
                    .iter()
                    .filter(|handle_use| handle_use.projection == "position")
                {
                    self.diagnostics.push(CompileError::new(
                        "SHREF003",
                        format!("ref `{name}` uses position without an anchor projection"),
                        handle_use.span,
                    ));
                }
            }
            let has_container = uses
                .iter()
                .any(|handle_use| handle_use.projection == "container");
            for handle_use in uses
                .iter()
                .filter(|handle_use| handle_use.projection == "container-query" && !has_container)
            {
                self.diagnostics.push(CompileError::new(
                    "SHREF006",
                    format!("ref `{name}` uses a container query without a container projection"),
                    handle_use.span,
                ));
            }
            let known_anchor_paths = uses
                .iter()
                .filter_map(|handle_use| {
                    (handle_use.projection == "anchor" || handle_use.projection == "ref")
                        .then_some(handle_use.jsx_path.as_ref())
                        .flatten()
                })
                .collect::<Vec<_>>();
            for handle_use in uses
                .iter()
                .filter(|handle_use| handle_use.projection == "selector")
            {
                let Some(path) = &handle_use.jsx_path else {
                    continue;
                };
                if !is_scoped
                    && !known_anchor_paths.is_empty()
                    && !known_anchor_paths
                        .iter()
                        .any(|anchor_path| is_strict_ancestor(anchor_path, path))
                {
                    self.diagnostics.push(CompileError::new(
                        "SHREF007",
                        format!("ref `{name}` selector has no known ancestor ref in this JSX tree"),
                        handle_use.span,
                    ));
                }
            }
        }

        self.validate_scoped_refs();
    }

    pub(super) fn validate_scoped_refs(&mut self) {
        for definition in self
            .handle_definitions
            .iter()
            .filter(|definition| definition.scope.is_some())
        {
            let scope = definition.scope.as_ref().expect("filtered above");
            let providers = self
                .scope_uses
                .iter()
                .filter(|use_| use_.name == scope.name.value)
                .collect::<Vec<_>>();
            if providers.is_empty() {
                self.diagnostics.push(CompileError::new(
                    "SHSCOPE002",
                    format!(
                        "ref `{}` is bound to scope `{}`, but that scope is not attached to a JSX element",
                        definition.name.value, scope.name.value
                    ),
                    definition.call_span,
                ));
                continue;
            }
            for reference_use in self.handle_uses.iter().filter(|use_| {
                use_.name == definition.name.value
                    && !matches!(use_.projection, "selector" | "container-query")
            }) {
                let Some(reference_path) = &reference_use.jsx_path else {
                    continue;
                };
                if !providers.iter().any(|provider| {
                    provider
                        .jsx_path
                        .as_ref()
                        .is_some_and(|path| is_strict_ancestor(path, reference_path))
                }) {
                    self.diagnostics.push(CompileError::new(
                        "SHSCOPE003",
                        format!(
                            "scope `{}` must be attached to an ancestor of ref `{}`",
                            scope.name.value, definition.name.value
                        ),
                        reference_use.span,
                    ));
                }
            }
            for selector_use in self
                .handle_uses
                .iter()
                .filter(|use_| use_.name == definition.name.value && use_.projection == "selector")
            {
                let Some(target_path) = &selector_use.jsx_path else {
                    continue;
                };
                if !providers.iter().any(|provider| {
                    provider
                        .jsx_path
                        .as_ref()
                        .is_some_and(|path| is_strict_ancestor(path, target_path))
                }) {
                    self.diagnostics.push(CompileError::new(
                        "SHSCOPE004",
                        format!(
                            "a selector using scoped ref `{}` must target an element inside scope `{}`",
                            definition.name.value, scope.name.value
                        ),
                        selector_use.span,
                    ));
                }
            }
        }
    }
}
