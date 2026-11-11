//! Light or dark: follow the desktop, or the user's override.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Follow,
    Light,
    Dark,
}

impl From<organiser_core::settings::ThemeChoice> for Mode {
    fn from(t: organiser_core::settings::ThemeChoice) -> Self {
        use organiser_core::settings::ThemeChoice as T;
        match t {
            T::Follow => Mode::Follow,
            T::Light => Mode::Light,
            T::Dark => Mode::Dark,
        }
    }
}

impl Mode {
    /// `desktop_dark` is what the desktop says now.
    pub fn is_dark(self, desktop_dark: bool) -> bool {
        match self {
            Mode::Follow => desktop_dark,
            Mode::Light => false,
            Mode::Dark => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_theme_maps_to_the_matching_mode() {
        use organiser_core::settings::ThemeChoice as T;
        assert_eq!(Mode::from(T::Follow), Mode::Follow);
        assert_eq!(Mode::from(T::Light), Mode::Light);
        assert_eq!(Mode::from(T::Dark), Mode::Dark);
    }

    #[test]
    fn override_beats_desktop() {
        assert!(Mode::Follow.is_dark(true));
        assert!(!Mode::Light.is_dark(true));
        assert!(Mode::Dark.is_dark(false));
    }
}
