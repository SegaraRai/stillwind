use serde::{Deserialize, Serialize};

use stillwind_design_system::{
    DesignSystemOptions, DesignSystemOptionsError, DesignSystemSnapshot,
};
use stillwind_source::normalized_module_path;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CompilerOptions {
    pub prefix: String,
    pub tailwind_prefix: String,
    pub forbid_tailwind_relationships: bool,
    pub class_namespace: String,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            prefix: "--sw-".to_owned(),
            tailwind_prefix: "--tw-".to_owned(),
            forbid_tailwind_relationships: false,
            class_namespace: String::new(),
        }
    }
}

impl CompilerOptions {
    /// A utility class belongs to one call and one branch, never to shared content.
    pub(crate) fn style_class_input(&self, module: &str, call: u32, branch: &str) -> String {
        let module = normalized_module_path(module);
        format!(
            "stillwind/style/v1:{}:{}{}:{module}:{call}:{branch}",
            self.class_namespace.len(),
            self.class_namespace,
            module.len(),
        )
    }

    pub fn design_system(&self) -> DesignSystemOptions {
        DesignSystemOptions {
            prefix: self.prefix.clone(),
            tailwind_prefix: self.tailwind_prefix.clone(),
        }
    }

    /// # Errors
    /// Returns invalid or conflicting property prefixes.
    pub fn validate(&self) -> Result<(), DesignSystemOptionsError> {
        self.design_system().validate()
    }

    pub(crate) fn resolved(self, snapshot: &DesignSystemSnapshot) -> Self {
        Self {
            prefix: snapshot.config().prefix.clone(),
            tailwind_prefix: snapshot.config().tailwind_prefix.clone(),
            ..self
        }
    }

    pub(crate) fn rewrite_generated(&self, value: &str) -> String {
        value
            .replace("--sw-", "\u{e000}stillwind-prefix\u{e001}")
            .replace("--tw-", "\u{e000}tailwind-prefix\u{e001}")
            .replace("\u{e000}stillwind-prefix\u{e001}", &self.prefix)
            .replace("\u{e000}tailwind-prefix\u{e001}", &self.tailwind_prefix)
    }
}
