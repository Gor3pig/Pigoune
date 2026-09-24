mod glycin_validator;
mod service;
mod svg;

pub use glycin_validator::{GlycinValidator, ImportValidationError};
pub use service::{DuplicatePolicy, ImportError, ImportOutcome, ImportService};

use pigoune_core::{ContainerMetadata, ImageMetadata};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportWarning {
    SvgExternalReferences,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ValidatedImport {
    pub metadata: ImageMetadata,
    pub warnings: Vec<ImportWarning>,
    pub container: Option<ContainerMetadata>,
}
