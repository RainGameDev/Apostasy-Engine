use apostasy_macros::Resource;
use winit::window::CursorGrabMode;

use crate::objects::resources::window_manager::WindowManager;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorLockMode {
    #[default]
    NoneVisible,
    NoneHidden,
    ConfinedHidden,
    ConfinedVisible,
    LockedHidden,
    LockedVisible,
}

#[derive(Resource, Clone, Default)]
pub struct CursorManager {
    pub cursor_lock_mode: CursorLockMode,
}

#[allow(unused_must_use)]
impl CursorManager {
    pub fn update_cursor(&self, window_manager: &mut WindowManager) {
        match self.cursor_lock_mode {
            CursorLockMode::NoneVisible => {
                window_manager.windows[&window_manager.primary_window_id].set_cursor_visible(true);
                window_manager.windows[&window_manager.primary_window_id]
                    .set_cursor_grab(CursorGrabMode::None);
            }
            CursorLockMode::NoneHidden => {
                window_manager.windows[&window_manager.primary_window_id].set_cursor_visible(false);
                window_manager.windows[&window_manager.primary_window_id]
                    .set_cursor_grab(CursorGrabMode::None);
            }

            CursorLockMode::ConfinedHidden => {
                window_manager.windows[&window_manager.primary_window_id].set_cursor_visible(false);
                window_manager.windows[&window_manager.primary_window_id]
                    .set_cursor_grab(CursorGrabMode::Confined);
            }

            CursorLockMode::ConfinedVisible => {
                window_manager.windows[&window_manager.primary_window_id].set_cursor_visible(true);
                window_manager.windows[&window_manager.primary_window_id]
                    .set_cursor_grab(CursorGrabMode::Confined);
            }

            CursorLockMode::LockedHidden => {
                window_manager.windows[&window_manager.primary_window_id].set_cursor_visible(false);
                window_manager.windows[&window_manager.primary_window_id]
                    .set_cursor_grab(CursorGrabMode::Locked);
            }

            CursorLockMode::LockedVisible => {
                window_manager.windows[&window_manager.primary_window_id].set_cursor_visible(true);
                window_manager.windows[&window_manager.primary_window_id]
                    .set_cursor_grab(CursorGrabMode::Locked);
            }
        }
    }

    /// If the current mode is unlocked, then lock it, otherwise unlock it
    pub fn set_mode(&mut self, mode: CursorLockMode) {
        self.cursor_lock_mode = mode;
    }

    /// If the current mode is unlocked, then lock it, otherwise unlock it
    pub fn switch_mode(&mut self) {
        match self.cursor_lock_mode {
            CursorLockMode::NoneVisible => self.cursor_lock_mode = CursorLockMode::LockedHidden,
            CursorLockMode::NoneHidden => self.cursor_lock_mode = CursorLockMode::LockedHidden,

            CursorLockMode::ConfinedHidden => self.cursor_lock_mode = CursorLockMode::NoneVisible,
            CursorLockMode::ConfinedVisible => self.cursor_lock_mode = CursorLockMode::NoneVisible,

            CursorLockMode::LockedHidden => self.cursor_lock_mode = CursorLockMode::NoneVisible,
            CursorLockMode::LockedVisible => self.cursor_lock_mode = CursorLockMode::NoneVisible,
        }
    }

    pub fn grab_cursor(&mut self, window_manager: &mut WindowManager) {
        window_manager.windows[&window_manager.primary_window_id].set_cursor_visible(false);
        let _ = window_manager.windows[&window_manager.primary_window_id]
            .set_cursor_grab(CursorGrabMode::Locked)
            .or_else(|_| {
                window_manager.windows[&window_manager.primary_window_id]
                    .set_cursor_grab(CursorGrabMode::Locked)
            });
    }
}
