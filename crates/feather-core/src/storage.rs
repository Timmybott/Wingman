//! The reserved storage backend.
//!
//! Feather keeps a Pterodactyl server behind the scenes to hold commit
//! snapshots and rollbacks (see M22). The API key for that server never lives
//! in the app — all file operations go through a server-side proxy. What *does*
//! live here is the server's identity, so the app can make one hard guarantee:
//!
//!   This server is completely excluded from every normal Feather code path.
//!
//! Even if a user connects a panel at the same host with a key that happens to
//! have access to it, this server is never listed, never importable, and never
//! deployed to as an ordinary server. The values below are not secrets (the
//! host is public and the short id only addresses the file API); only the API
//! key is sensitive, and it is not here.

use url::Url;

/// Host of the panel that fronts the storage backend.
pub const STORAGE_PANEL_HOST: &str = "panel.spaceify.eu";

/// Short identifier of the storage server on that panel. Pterodactyl's short
/// identifier is the first group of the server UUID.
pub const STORAGE_SERVER_ID: &str = "893a2ffd";

/// Whether a server (by short `identifier` and full `uuid`) on the panel at
/// `host` is the reserved storage backend and must be hidden from every normal
/// server view, import and deploy path.
pub fn is_reserved_storage_server(host: Option<&str>, identifier: &str, uuid: &str) -> bool {
    if !host_is_storage_panel(host) {
        return false;
    }
    identifier.eq_ignore_ascii_case(STORAGE_SERVER_ID)
        || uuid
            .split('-')
            .next()
            .is_some_and(|first| first.eq_ignore_ascii_case(STORAGE_SERVER_ID))
}

/// Whether `host` is the storage panel's host (case-insensitive).
pub fn host_is_storage_panel(host: Option<&str>) -> bool {
    host.is_some_and(|h| h.eq_ignore_ascii_case(STORAGE_PANEL_HOST))
}

/// Whether a server identified only by its short `identifier` on the panel at
/// `base` is the reserved backend. Used to guard server-scoped operations where
/// only the identifier is known.
pub fn is_reserved_on(base: &Url, identifier: &str) -> bool {
    host_is_storage_panel(base.host_str()) && identifier.eq_ignore_ascii_case(STORAGE_SERVER_ID)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_by_identifier_on_storage_host() {
        assert!(is_reserved_storage_server(
            Some("panel.spaceify.eu"),
            "893a2ffd",
            "893a2ffd-1111-2222-3333-444455556666",
        ));
    }

    #[test]
    fn matches_case_insensitively() {
        assert!(is_reserved_storage_server(
            Some("Panel.Spaceify.EU"),
            "893A2FFD",
            "unused",
        ));
    }

    #[test]
    fn matches_by_uuid_prefix() {
        assert!(is_reserved_storage_server(
            Some("panel.spaceify.eu"),
            "somethingelse",
            "893a2ffd-aaaa-bbbb-cccc-dddddddddddd",
        ));
    }

    #[test]
    fn other_server_on_same_host_is_allowed() {
        assert!(!is_reserved_storage_server(
            Some("panel.spaceify.eu"),
            "d3aac109",
            "d3aac109-1111-2222-3333-444455556666",
        ));
    }

    #[test]
    fn same_id_on_a_different_host_is_allowed() {
        // A short-id collision on an unrelated panel is a different server.
        assert!(!is_reserved_storage_server(
            Some("panel.example.com"),
            "893a2ffd",
            "893a2ffd-1111-2222-3333-444455556666",
        ));
    }

    #[test]
    fn is_reserved_on_checks_host_and_id() {
        let storage = Url::parse("https://panel.spaceify.eu/").unwrap();
        let other = Url::parse("https://panel.example.com/").unwrap();
        assert!(is_reserved_on(&storage, "893a2ffd"));
        assert!(!is_reserved_on(&storage, "d3aac109"));
        assert!(!is_reserved_on(&other, "893a2ffd"));
    }
}
