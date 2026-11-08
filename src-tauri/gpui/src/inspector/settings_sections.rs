//! Disabled host-specific sections keep their row and the reason the native
//! host cannot back them.

use super::settings_page::{settings_row, unavailable_control};
use crate::settings::Row;
use gpui_kit::{AnyElement, SharedString, div, prelude::*};

pub(super) fn section(row: &'static Row) -> AnyElement {
    let control = unavailable_control(
        SharedString::from(format!("setting-control:{}", row.title)),
        row.unavailable
            .expect("host sections have an unavailable reason"),
        div(),
    );
    settings_row(row.title, row.description.into(), control).into_any_element()
}
