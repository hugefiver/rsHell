use crate::{
    EditorValidationError,
    connection_editor_diagnostics::{EditorRejection, rejection_line},
};

#[test]
fn editor_rejection_lines_are_static_and_redacted() {
    assert_eq!(
        rejection_line(EditorRejection::Validation(
            EditorValidationError::SecretRequired
        )),
        "P0_EDITOR source=validation code=secret_required"
    );
    assert_eq!(
        rejection_line(EditorRejection::Operation),
        "P0_EDITOR source=application code=operation_failed"
    );
}
