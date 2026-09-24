mod glycin_validator;
mod svg;

pub use glycin_validator::{GlycinValidator, ImportValidationError};

use pigoune_core::ImageMetadata;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportWarning {
    SvgExternalReferences,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ValidatedImport {
    pub metadata: ImageMetadata,
    pub warnings: Vec<ImportWarning>,
}
