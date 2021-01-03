//! Configure your application;

use crate::RenderSettings;
use baseview::WindowOpenOptions;

/// The settings of an application.
pub struct Settings {
    /// The `baseview` window settings.
    pub window: WindowOpenOptions,

    /// The settings for the rendering backend.
    pub render_settings: RenderSettings,
}
