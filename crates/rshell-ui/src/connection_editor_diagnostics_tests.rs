use crate::{
    EditorValidationError,
    connection_editor_diagnostics::{EditorRejection, rejection_line},
};
use rshell_core::AppFailureCategory;

#[test]
fn editor_rejection_lines_are_static_and_redacted() {
    assert_eq!(
        rejection_line(EditorRejection::Validation(
            EditorValidationError::SecretRequired
        )),
        "P0_EDITOR source=validation code=secret_required"
    );
    assert_eq!(
        rejection_line(EditorRejection::Operation(AppFailureCategory::Vault)),
        "P0_EDITOR source=application code=vault"
    );
    assert_eq!(
        rejection_line(EditorRejection::Operation(AppFailureCategory::Storage)),
        "P0_EDITOR source=application code=storage"
    );
    assert_eq!(
        rejection_line(EditorRejection::Operation(AppFailureCategory::Backpressure)),
        "P0_EDITOR source=application code=backpressure"
    );
}
