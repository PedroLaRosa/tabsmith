use crate::completion::domain::value_objects::annotation_kind::AnnotationKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TokenAnnotation {
    /// For a composite token: the kind of its last sub-token.
    pub(crate) kind: AnnotationKind,
    pub(crate) text: String,
    /// Sub-tokens of a composite token (`-abc`, `--opt=val`), else empty.
    pub(crate) parts: Vec<(AnnotationKind, String)>,
}
