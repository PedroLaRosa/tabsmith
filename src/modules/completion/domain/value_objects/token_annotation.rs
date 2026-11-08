use crate::modules::completion::domain::value_objects::annotation_kind::AnnotationKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TokenAnnotation {
    /// For a composite token: the kind of its last sub-token.
    kind: AnnotationKind,
    text: String,
    /// Sub-tokens of a composite token (`-abc`, `--opt=val`), else empty.
    parts: Vec<(AnnotationKind, String)>,
}

impl TokenAnnotation {
    pub(crate) fn new(kind: AnnotationKind, text: String, parts: Vec<(AnnotationKind, String)>) -> Self {
        TokenAnnotation { kind, text, parts }
    }

    pub(crate) fn kind(&self) -> AnnotationKind {
        self.kind
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn parts(&self) -> &Vec<(AnnotationKind, String)> {
        &self.parts
    }
}
