/*
 * Copyright © 2023 Collabora Ltd.
 * Copyright © 2024 Valve Software
 * Copyright © 2024 Igalia S.L.
 *
 * SPDX-License-Identifier: MIT
 */

#[cfg(test)]
use anyhow::anyhow;
use anyhow::{bail, ensure, Error, Result};
use tokio::fs::try_exists;
#[cfg(test)]
use tokio::fs::{create_dir_all, write};
use zbus::Connection;

use crate::path;
use crate::systemd::SystemdUnit;

const SESSION_CHECK_PATH: &str = "/etc/sddm.conf.d/steamos.conf";

#[derive(PartialEq, Debug, Copy, Clone)]
pub(crate) enum WindowingSystem {
    X11,
    Wayland,
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub(crate) enum SessionType {
    Gamescope,
    Plasma(WindowingSystem),
}

#[derive(Debug)]
pub(crate) struct SessionManager {
    connection: Connection,
}

impl From<SessionType> for u32 {
    fn from(val: SessionType) -> u32 {
        match val {
            SessionType::Gamescope => 0,
            SessionType::Plasma(WindowingSystem::X11) => 1,
            SessionType::Plasma(WindowingSystem::Wayland) => 2,
        }
    }
}

impl TryFrom<u32> for SessionType {
    type Error = Error;

    fn try_from(val: u32) -> Result<SessionType> {
        match val {
            0 => Ok(SessionType::Gamescope),
            1 => Ok(SessionType::Plasma(WindowingSystem::X11)),
            2 => Ok(SessionType::Plasma(WindowingSystem::Wayland)),
            v => bail!("No enum match for value {v}"),
        }
    }
}

pub(crate) async fn is_session_managed() -> Result<bool> {
    Ok(try_exists(path(SESSION_CHECK_PATH)).await?)
}

#[cfg(test)]
pub(crate) async fn make_managed() -> Result<()> {
    let check_path = path(SESSION_CHECK_PATH);
    create_dir_all(check_path.parent().ok_or(anyhow!("Couldn't make dir"))?).await?;
    write(check_path, "").await?;
    Ok(())
}

impl SessionManager {
    pub(crate) fn new(connection: Connection) -> SessionManager {
        SessionManager { connection }
    }

    pub(crate) async fn switch_to_session(&self, session: SessionType) -> Result<()> {
        match session {
            SessionType::Gamescope => todo!(),
            SessionType::Plasma(WindowingSystem::X11) => todo!(),
            SessionType::Plasma(WindowingSystem::Wayland) => todo!(),
        }
    }

    pub(crate) async fn default_desktop_type(&self) -> Result<SessionType> {
        todo!();
    }

    pub(crate) async fn set_default_desktop_type(&self, session: SessionType) -> Result<()> {
        ensure!(
            !matches!(session, SessionType::Gamescope),
            "Not a desktop session type"
        );
        todo!();
    }

    pub(crate) async fn default_session_type(&self) -> Result<SessionType> {
        todo!();
    }

    pub(crate) async fn set_default_session_type(&self, session: SessionType) -> Result<()> {
        todo!();
    }

    pub(crate) async fn current_session_type(&self) -> Result<SessionType> {
        let gamescope =
            SystemdUnit::new(self.connection.clone(), "gamescope-session.service").await?;
        if gamescope.active().await? {
            return Ok(SessionType::Gamescope);
        }

        let plasma_x11 =
            SystemdUnit::new(self.connection.clone(), "plasma-kwin_x11.service").await?;
        if plasma_x11.active().await? {
            return Ok(SessionType::Plasma(WindowingSystem::X11));
        }
        let plasma_wayland =
            SystemdUnit::new(self.connection.clone(), "plasma-kwin_wayland.service").await?;
        if plasma_wayland.active().await? {
            return Ok(SessionType::Plasma(WindowingSystem::Wayland));
        }
        bail!("No session active");
    }
}
