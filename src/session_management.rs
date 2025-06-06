/*
 * Copyright © 2025 Harald Sitter <sitter@kde.org>
 *
 * SPDX-License-Identifier: MIT
 */

use std::path::Path;

use anyhow::{anyhow, bail, ensure, Result};
use tokio::fs;
use zbus::{self, Connection};

use ini::Ini;

use crate::systemd::SystemdUnit;

// This is our persistent store. It applies unless an ephemeral session is set.
const PERSISTENT_CONFIG_FILE: &str = "/etc/sddm.conf.d/yy-steamos-session.conf";
// The ephemeral session configuration is ordered AFTER the persistent one so it can temporarily override it.
const EPHEMERAL_CONFIG_FILE: &str = "/etc/sddm.conf.d/zz-steamos-autologin.conf";
const CONFIG_SECTION: &str = "X-SteamOS";
const DEFAULT_SESSION: &str = "unknown";
const CONFIG_KEY_DEFAULT_DESKTOP_SESSION: &str = "DefaultDesktopSession";
const CONFIG_KEY_DEFAULT_SESSION: &str = "DefaultSession";

async fn find_type_in_dir(prefix: impl AsRef<Path>, ty: &str) -> Result<String> {
    let type_without_suffix = ty.trim_end_matches(".desktop");
    let expected_session = format!("{}.desktop", type_without_suffix);
    let expected_one_shot = match type_without_suffix {
        // Plasma desktop files are inconsistently named, so we have to special case them.
        // For everything else the rule set forth is simply: <type>-steamos-oneshot.desktop
        "plasma" => "plasma-steamos-wayland-oneshot.desktop".to_string(),
        "plasmax11" => "plasma-steamos-oneshot.desktop".to_string(),
        _ => format!("{}-steamos-oneshot.desktop", type_without_suffix),
    };

    let mut exists = false;
    let mut has_one_shot = false;

    let mut dir = fs::read_dir(prefix.as_ref()).await?;
    while let Some(entry) = dir.next_entry().await? {
        let file_name = entry.file_name();
        let session: &str = &file_name.to_string_lossy();
        if session == &expected_session {
            exists = true;
        }
        if session == &expected_one_shot {
            has_one_shot = true;
        }
    }

    ensure!(
        exists || has_one_shot,
        "Session type {ty} not found in directory {}",
        prefix.as_ref().display()
    );
    Ok(if has_one_shot {
        expected_one_shot
    } else {
        expected_session
    })
}

async fn locate_session(ty: &str) -> Result<String> {
    // Guard against bad input strings. Notably we don't want relative paths here as they would allow inspecting
    // all root owned files.
    // While we are at it also figure out if the session type has a one-shot variant and prefer that.

    for dir in ["/usr/share/xsessions", "/usr/share/wayland-sessions"] {
        match find_type_in_dir(dir, ty).await {
            Ok(session) => {
                return Ok(session);
            }
            Err(_) => {
                // Try next
            }
        }
    }

    bail!("Session type {ty} not found in any of the known session directories")
}

async fn read_session(config_key: &str) -> String {
    let config = match Ini::load_from_file(PERSISTENT_CONFIG_FILE) {
        Ok(c) => c,
        Err(_) => {
            return DEFAULT_SESSION.to_owned();
        }
    };
    match config.section(Some(CONFIG_SECTION)) {
        Some(section) => {
            let session = section.get(config_key);
            return session.unwrap_or(DEFAULT_SESSION).to_owned();
        }
        None => return DEFAULT_SESSION.to_owned(),
    }
}

async fn write_type_to_config(
    config_file: &str,
    config_section: &str,
    config_key: &str,
    session_name: &str,
) -> Result<()> {
    let mut config = Ini::new();
    if Path::new(config_file).exists() {
        Ini::load_from_file(config_file)?;
    };
    config
        .with_section(Some(config_section))
        .set(config_key, session_name);
    config.write_to_file(config_file)?;
    Ok(())
}

async fn write_session(ty: &str, config_key: &str) -> Result<()> {
    // Note that we are writing to our config. We want to store the verbatim session name not its resolved name!
    // The resolved name may be a oneshot session, but the fact that we pick that is an implementation detail.
    let _dont_care_about_real_name = locate_session(ty);
    write_type_to_config(PERSISTENT_CONFIG_FILE, CONFIG_SECTION, config_key, ty).await
}

async fn set_autologin_session(ty: &str, config_file: &str) -> Result<()> {
    let session_name = locate_session(ty).await?;
    write_type_to_config(config_file, "Autologin", "Session", &session_name).await
}

pub(crate) async fn switch_to_session(ty: &str) -> Result<()> {
    set_autologin_session(ty, EPHEMERAL_CONFIG_FILE).await
}

pub(crate) async fn read_default_desktop_session_type() -> String {
    read_session(CONFIG_KEY_DEFAULT_DESKTOP_SESSION).await
}

pub(crate) async fn write_default_desktop_session_type(ty: &str) -> Result<()> {
    write_session(ty, CONFIG_KEY_DEFAULT_DESKTOP_SESSION).await
}

pub(crate) async fn read_default_session_type() -> String {
    read_session(CONFIG_KEY_DEFAULT_SESSION).await
}

pub(crate) async fn write_default_session_type(ty: &str) -> Result<()> {
    write_session(ty, CONFIG_KEY_DEFAULT_SESSION).await
}

pub(crate) async fn restart_session() -> Result<()> {
    for service in ["plasma-workspace.target", "gamescope-session.service"] {
        let connection = Connection::session().await?;
        let unit = SystemdUnit::new(connection, service).await?;
        match unit.stop().await {
            Ok(_) => (),
            Err(e) => return Err(anyhow!("Failed to stop {}: {}", service, e)),
        };
    }

    Ok(())
}
