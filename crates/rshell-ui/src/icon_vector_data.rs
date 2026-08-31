use crate::ProductIcon;

pub(crate) const VIEWBOX_SIZE: f64 = 16.0;

#[derive(Clone, Copy)]
pub(crate) enum VectorOp {
    MoveTo(f64, f64),
    LineTo(f64, f64),
    Arc(f64, f64, f64, f64, f64),
    Close,
}

const IMPORT: &[VectorOp] = &[
    VectorOp::MoveTo(8.0, 2.0),
    VectorOp::LineTo(8.0, 9.0),
    VectorOp::LineTo(11.0, 6.0),
    VectorOp::MoveTo(8.0, 9.0),
    VectorOp::LineTo(5.0, 6.0),
    VectorOp::MoveTo(3.0, 10.0),
    VectorOp::LineTo(3.0, 13.0),
    VectorOp::LineTo(13.0, 13.0),
    VectorOp::LineTo(13.0, 10.0),
];
const SETTINGS: &[VectorOp] = &[
    VectorOp::MoveTo(3.0, 4.0),
    VectorOp::LineTo(13.0, 4.0),
    VectorOp::MoveTo(3.0, 8.0),
    VectorOp::LineTo(13.0, 8.0),
    VectorOp::MoveTo(3.0, 12.0),
    VectorOp::LineTo(13.0, 12.0),
    VectorOp::MoveTo(5.0, 2.0),
    VectorOp::LineTo(5.0, 6.0),
    VectorOp::MoveTo(11.0, 6.0),
    VectorOp::LineTo(11.0, 10.0),
    VectorOp::MoveTo(7.0, 10.0),
    VectorOp::LineTo(7.0, 14.0),
];
const ADD_CONNECTION: &[VectorOp] = &[
    VectorOp::MoveTo(2.0, 3.5),
    VectorOp::LineTo(10.0, 3.5),
    VectorOp::LineTo(10.0, 9.5),
    VectorOp::LineTo(2.0, 9.5),
    VectorOp::Close,
    VectorOp::MoveTo(4.0, 12.0),
    VectorOp::LineTo(8.0, 12.0),
    VectorOp::MoveTo(12.5, 2.0),
    VectorOp::LineTo(12.5, 7.0),
    VectorOp::MoveTo(10.0, 4.5),
    VectorOp::LineTo(15.0, 4.5),
];
const ADD_GROUP: &[VectorOp] = &[
    VectorOp::MoveTo(2.0, 4.0),
    VectorOp::LineTo(7.0, 4.0),
    VectorOp::LineTo(8.5, 5.0),
    VectorOp::LineTo(14.0, 5.0),
    VectorOp::LineTo(14.0, 12.0),
    VectorOp::LineTo(2.0, 12.0),
    VectorOp::Close,
    VectorOp::MoveTo(10.5, 7.0),
    VectorOp::LineTo(10.5, 10.0),
    VectorOp::MoveTo(9.0, 8.5),
    VectorOp::LineTo(12.0, 8.5),
];
const EDIT: &[VectorOp] = &[
    VectorOp::MoveTo(3.0, 11.5),
    VectorOp::LineTo(3.5, 9.0),
    VectorOp::LineTo(10.0, 2.5),
    VectorOp::LineTo(13.5, 6.0),
    VectorOp::LineTo(7.0, 12.5),
    VectorOp::LineTo(3.0, 13.0),
    VectorOp::Close,
    VectorOp::MoveTo(9.0, 3.5),
    VectorOp::LineTo(12.5, 7.0),
];
const DUPLICATE: &[VectorOp] = &[
    VectorOp::MoveTo(5.0, 5.0),
    VectorOp::LineTo(13.0, 5.0),
    VectorOp::LineTo(13.0, 13.0),
    VectorOp::LineTo(5.0, 13.0),
    VectorOp::Close,
    VectorOp::MoveTo(3.0, 11.0),
    VectorOp::LineTo(2.0, 11.0),
    VectorOp::LineTo(2.0, 2.0),
    VectorOp::LineTo(11.0, 2.0),
    VectorOp::LineTo(11.0, 3.0),
];
const DELETE: &[VectorOp] = &[
    VectorOp::MoveTo(3.0, 4.0),
    VectorOp::LineTo(13.0, 4.0),
    VectorOp::MoveTo(6.0, 4.0),
    VectorOp::LineTo(6.0, 2.0),
    VectorOp::LineTo(10.0, 2.0),
    VectorOp::LineTo(10.0, 4.0),
    VectorOp::MoveTo(4.0, 4.0),
    VectorOp::LineTo(4.5, 14.0),
    VectorOp::LineTo(11.5, 14.0),
    VectorOp::LineTo(12.0, 4.0),
];
const CLOSE_TAB: &[VectorOp] = &[
    VectorOp::MoveTo(4.0, 4.0),
    VectorOp::LineTo(12.0, 12.0),
    VectorOp::MoveTo(12.0, 4.0),
    VectorOp::LineTo(4.0, 12.0),
];
const NEW_TAB: &[VectorOp] = &[
    VectorOp::MoveTo(2.0, 3.0),
    VectorOp::LineTo(10.0, 3.0),
    VectorOp::LineTo(10.0, 13.0),
    VectorOp::LineTo(2.0, 13.0),
    VectorOp::Close,
    VectorOp::MoveTo(13.0, 5.0),
    VectorOp::LineTo(13.0, 11.0),
    VectorOp::MoveTo(10.0, 8.0),
    VectorOp::LineTo(16.0, 8.0),
];
const SPLIT_HORIZONTAL: &[VectorOp] = &[
    VectorOp::MoveTo(2.0, 3.0),
    VectorOp::LineTo(14.0, 3.0),
    VectorOp::LineTo(14.0, 13.0),
    VectorOp::LineTo(2.0, 13.0),
    VectorOp::Close,
    VectorOp::MoveTo(8.0, 3.0),
    VectorOp::LineTo(8.0, 13.0),
];
const SPLIT_VERTICAL: &[VectorOp] = &[
    VectorOp::MoveTo(2.0, 3.0),
    VectorOp::LineTo(14.0, 3.0),
    VectorOp::LineTo(14.0, 13.0),
    VectorOp::LineTo(2.0, 13.0),
    VectorOp::Close,
    VectorOp::MoveTo(2.0, 8.0),
    VectorOp::LineTo(14.0, 8.0),
];
const RETRY: &[VectorOp] = &[
    VectorOp::Arc(8.0, 8.0, 5.0, 0.35, 5.7),
    VectorOp::MoveTo(12.5, 2.0),
    VectorOp::LineTo(12.5, 6.0),
    VectorOp::LineTo(8.5, 6.0),
];
const COPY_DIAGNOSTICS: &[VectorOp] = &[
    VectorOp::MoveTo(2.0, 3.0),
    VectorOp::LineTo(10.0, 3.0),
    VectorOp::LineTo(10.0, 11.0),
    VectorOp::LineTo(2.0, 11.0),
    VectorOp::Close,
    VectorOp::MoveTo(5.0, 6.0),
    VectorOp::LineTo(14.0, 6.0),
    VectorOp::LineTo(14.0, 14.0),
    VectorOp::LineTo(5.0, 14.0),
    VectorOp::Close,
    VectorOp::MoveTo(8.0, 8.0),
    VectorOp::LineTo(8.0, 11.0),
];
const WARNING: &[VectorOp] = &[
    VectorOp::MoveTo(8.0, 2.0),
    VectorOp::LineTo(15.0, 14.0),
    VectorOp::LineTo(1.0, 14.0),
    VectorOp::Close,
    VectorOp::MoveTo(8.0, 6.0),
    VectorOp::LineTo(8.0, 10.0),
    VectorOp::MoveTo(8.0, 12.0),
    VectorOp::LineTo(8.01, 12.0),
];
const SECRET_PRESENT: &[VectorOp] = &[
    VectorOp::Arc(8.0, 5.0, 3.0, std::f64::consts::PI, std::f64::consts::TAU),
    VectorOp::MoveTo(5.0, 5.0),
    VectorOp::LineTo(5.0, 7.0),
    VectorOp::MoveTo(11.0, 5.0),
    VectorOp::LineTo(11.0, 7.0),
    VectorOp::MoveTo(4.0, 7.0),
    VectorOp::LineTo(12.0, 7.0),
    VectorOp::LineTo(12.0, 14.0),
    VectorOp::LineTo(4.0, 14.0),
    VectorOp::Close,
    VectorOp::MoveTo(8.0, 10.0),
    VectorOp::LineTo(8.0, 12.0),
];
const HOST_TRUST: &[VectorOp] = &[
    VectorOp::MoveTo(8.0, 2.0),
    VectorOp::LineTo(13.0, 4.0),
    VectorOp::LineTo(13.0, 8.0),
    VectorOp::LineTo(12.0, 11.0),
    VectorOp::LineTo(8.0, 14.0),
    VectorOp::LineTo(4.0, 11.0),
    VectorOp::LineTo(3.0, 8.0),
    VectorOp::LineTo(3.0, 4.0),
    VectorOp::Close,
    VectorOp::MoveTo(6.0, 8.0),
    VectorOp::LineTo(7.5, 9.5),
    VectorOp::LineTo(10.5, 6.5),
];
const MORE: &[VectorOp] = &[
    VectorOp::MoveTo(4.0, 8.0),
    VectorOp::LineTo(4.01, 8.0),
    VectorOp::MoveTo(8.0, 8.0),
    VectorOp::LineTo(8.01, 8.0),
    VectorOp::MoveTo(12.0, 8.0),
    VectorOp::LineTo(12.01, 8.0),
];
const NAVIGATION: &[VectorOp] = &[
    VectorOp::MoveTo(3.0, 4.0),
    VectorOp::LineTo(13.0, 4.0),
    VectorOp::MoveTo(3.0, 8.0),
    VectorOp::LineTo(13.0, 8.0),
    VectorOp::MoveTo(3.0, 12.0),
    VectorOp::LineTo(13.0, 12.0),
];

pub(crate) fn operations(icon: ProductIcon) -> &'static [VectorOp] {
    match icon {
        ProductIcon::Import => IMPORT,
        ProductIcon::Settings => SETTINGS,
        ProductIcon::AddConnection => ADD_CONNECTION,
        ProductIcon::AddGroup => ADD_GROUP,
        ProductIcon::Edit => EDIT,
        ProductIcon::Duplicate => DUPLICATE,
        ProductIcon::Delete => DELETE,
        ProductIcon::CloseTab => CLOSE_TAB,
        ProductIcon::NewTab => NEW_TAB,
        ProductIcon::SplitHorizontal => SPLIT_HORIZONTAL,
        ProductIcon::SplitVertical => SPLIT_VERTICAL,
        ProductIcon::Retry => RETRY,
        ProductIcon::CopyDiagnostics => COPY_DIAGNOSTICS,
        ProductIcon::Warning => WARNING,
        ProductIcon::SecretPresent => SECRET_PRESENT,
        ProductIcon::HostTrust => HOST_TRUST,
        ProductIcon::More => MORE,
        ProductIcon::Navigation => NAVIGATION,
    }
}
