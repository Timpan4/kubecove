//! Disabled host-specific sections keep their row and the reason the native
//! host cannot back them. Server information shows the connected remote.

use super::settings_page::{settings_row, unavailable_control};
use super::surface::secondary_badge;
use crate::backend::RemoteConnectionSummary;
use crate::settings::Row;
use crate::targets::TestSupportExt;
use crate::ui;
use gpui_kit::{AnyElement, SharedString, div, prelude::*, px};

pub(super) fn section(row: &'static Row) -> AnyElement {
    let control = unavailable_control(
        SharedString::from(format!("setting-control:{}", row.title)),
        row.unavailable
            .expect("host sections have an unavailable reason"),
        div(),
    );
    settings_row(row.title, row.description.into(), control).into_any_element()
}

/// `credentialStoreLabel` in `SettingsSurface.svelte`.
fn credential_store_label(store: Option<&str>) -> String {
    let Some(store) = store.filter(|store| !store.is_empty()) else {
        return "Not reported".to_owned();
    };
    let normalized = store.to_lowercase();
    if normalized.contains("encrypted") || normalized.contains("file") {
        "Encrypted file storage".to_owned()
    } else if normalized.contains("native") || normalized.contains("keyring") {
        "Native credential storage".to_owned()
    } else {
        store.to_owned()
    }
}

/// The browser runtime's server row, for the remote this desktop is connected to.
pub(super) fn server_information(
    row: &'static Row,
    remote: &RemoteConnectionSummary,
) -> AnyElement {
    let version = format!("Server {}", remote.server_version);
    let store = credential_store_label(remote.credential_store.as_deref());
    let control = div()
        .flex()
        .gap(px(8.))
        .child(
            ui::outline_badge(version.clone())
                .id("server-information:version")
                .test_support()
                .aria_label(version),
        )
        .child(
            secondary_badge(store.clone())
                .id("server-information:credential-store")
                .test_support()
                .aria_label(store),
        );
    settings_row(row.title, row.description.into(), control.into_any_element()).into_any_element()
}
