//! Startup / shutdown orchestration.

use windows::core::Result;

use crate::ui::window;
use crate::win::SingleInstance;

/// Name of the per-session mutex that enforces a single running instance.
const INSTANCE_MUTEX: &str = "Local\\WinHelper.SingleInstance";

pub fn run() -> Result<()> {
    // Held for the lifetime of the process; released on drop.
    let Some(_instance) = SingleInstance::acquire(INSTANCE_MUTEX)? else {
        window::activate_existing();
        return Ok(());
    };

    let _hwnd = window::create()?;
    window::run_message_loop();
    Ok(())
}
