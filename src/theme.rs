/// Design System Constants - Dark Gray Slate Glass Theme
pub mod colors {
    use iced::Color;

    // Glassmorphism base colors
    pub const GLASS_BASE: Color = Color::from_rgba(0.07, 0.07, 0.08, 0.96);
    // near-black slate

    // Shine effects (neutral, soft)
    pub const SHINE_WHITE: Color = Color::from_rgba(0.8, 0.8, 0.8, 0.08);
    pub const SHINE_TRANSPARENT: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.0);

    // UI Elements
    pub const BG_CARD: Color = Color::from_rgba(0.14, 0.15, 0.17, 0.92);
    pub const BG_CARD_HOVER: Color = Color::from_rgba(0.55, 0.60, 0.70, 0.18);
    pub const BG_INPUT: Color = Color::from_rgba(0.10, 0.11, 0.13, 0.75);

    // Accents (cool slate)
    pub const ACCENT_PRIMARY: Color = Color::from_rgb(0.65, 0.70, 0.80);
    pub const ACCENT_GLOW: Color = Color::from_rgba(0.65, 0.70, 0.80, 0.35);

    // Text
    pub const TEXT_PRIMARY: Color = Color::from_rgb(0.95, 0.95, 0.95);
    pub const TEXT_SECONDARY: Color = Color::from_rgb(0.72, 0.74, 0.78);

    // Borders
    pub const GLASS_BORDER: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.20);
    
    // Window Controls
    pub const CONTROL_CLOSE: Color = Color::from_rgb(0.9, 0.35, 0.35);
    pub const CONTROL_MIN: Color = Color::from_rgb(0.9, 0.7, 0.3);
    pub const CONTROL_MAX: Color = Color::from_rgb(0.3, 0.7, 0.4);
}

/// Custom Styles for Iced Widgets
pub mod styles {
    use iced::widget::{button, container, text_input};
    use iced::{Background, Color, Vector, Border, Shadow, gradient, Radians};
    use super::colors;

    /// Base layer of the glass
    pub fn glass_base(_theme: &iced::Theme) -> container::Style {
        container::Style {
            background: Some(Background::Color(colors::GLASS_BASE)),
            border: Border {
                color: colors::GLASS_BORDER,
                width: 1.0,
                radius: 20.0.into(),
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                offset: Vector::new(0.0, 12.0),
                blur_radius: 40.0,
            },
            ..Default::default()
        }
    }

    /// Top-down highlight
    pub fn glass_highlight_top(_theme: &iced::Theme) -> container::Style {
        let gradient = gradient::Linear::new(Radians(1.5708)) // 90 degrees
            .add_stop(0.0, colors::SHINE_WHITE)
            .add_stop(0.1, colors::SHINE_TRANSPARENT);

        container::Style {
            background: Some(Background::Gradient(iced::Gradient::Linear(gradient))),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 20.0.into(),
            },
            ..Default::default()
        }
    }

    /// Bottom-up highlight
    pub fn glass_highlight_bottom(_theme: &iced::Theme) -> container::Style {
        let gradient = gradient::Linear::new(Radians(4.7124)) // 270 degrees
            .add_stop(0.0, colors::SHINE_WHITE)
            .add_stop(0.1, colors::SHINE_TRANSPARENT);

        container::Style {
            background: Some(Background::Gradient(iced::Gradient::Linear(gradient))),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 20.0.into(),
            },
            ..Default::default()
        }
    }

    /// Left highlight
    pub fn glass_highlight_left(_theme: &iced::Theme) -> container::Style {
        let gradient = gradient::Linear::new(Radians(0.0)) // 0 degrees (left to right)
            .add_stop(0.0, colors::SHINE_WHITE)
            .add_stop(0.1, colors::SHINE_TRANSPARENT);

        container::Style {
            background: Some(Background::Gradient(iced::Gradient::Linear(gradient))),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 20.0.into(),
            },
            ..Default::default()
        }
    }

    /// Right highlight
    pub fn glass_highlight_right(_theme: &iced::Theme) -> container::Style {
        let gradient = gradient::Linear::new(Radians(3.1416)) // 180 degrees (right to left)
            .add_stop(0.0, colors::SHINE_WHITE)
            .add_stop(0.1, colors::SHINE_TRANSPARENT);

        container::Style {
            background: Some(Background::Gradient(iced::Gradient::Linear(gradient))),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 20.0.into(),
            },
            ..Default::default()
        }
    }

    /// Styled search input
    pub fn search_input(_theme: &iced::Theme, status: text_input::Status) -> text_input::Style {
        let base = text_input::Style {
            background: Background::Color(colors::BG_INPUT),
            border: Border {
                color: colors::GLASS_BORDER,
                width: 1.0,
                radius: 12.0.into(),
            },
            icon: colors::TEXT_SECONDARY,
            placeholder: colors::TEXT_SECONDARY,
            value: colors::TEXT_PRIMARY,
            selection: colors::ACCENT_PRIMARY,
        };

        match status {
            text_input::Status::Focused { .. } => text_input::Style {
                border: Border {
                    color: colors::ACCENT_PRIMARY,
                    width: 1.5,
                    radius: 12.0.into(),
                },
                ..base
            },
            _ => base,
        }
    }

    pub fn app_card(_theme: &iced::Theme, status: button::Status) -> button::Style {
        match status {
            button::Status::Active => button::Style {
                background: Some(Background::Color(colors::BG_CARD)),
                text_color: colors::TEXT_PRIMARY,
                border: Border {
                    color: colors::GLASS_BORDER,
                    width: 1.0,
                    radius: 14.0.into(),
                },
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.2),
                    offset: Vector::new(0.0, 4.0),
                    blur_radius: 8.0,
                },
                ..Default::default()
            },
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(colors::BG_CARD_HOVER)),
                text_color: colors::TEXT_PRIMARY,
                border: Border {
                    color: colors::ACCENT_PRIMARY,
                    width: 1.0,
                    radius: 14.0.into(),
                },
                shadow: Shadow {
                    color: colors::ACCENT_GLOW,
                    offset: Vector::new(0.0, 0.0),
                    blur_radius: 16.0,
                },
                ..Default::default()
            },
            button::Status::Pressed => button::Style {
                background: Some(Background::Color(colors::BG_CARD)),
                text_color: colors::TEXT_PRIMARY,
                border: Border {
                    color: colors::ACCENT_PRIMARY,
                    width: 1.0,
                    radius: 14.0.into(),
                },
                shadow: Shadow::default(),
                ..Default::default()
            },
            button::Status::Disabled => button::Style::default(),
        }
    }

    /// Window Control Button (Mac-style blobs)
    pub fn window_control(_theme: &iced::Theme, status: button::Status, color: Color) -> button::Style {
        let base = button::Style {
             background: Some(Background::Color(color)),
             border: Border {
                 color: Color::TRANSPARENT,
                 width: 0.0,
                 radius: 100.0.into(), // Circle
             },
             ..Default::default()
        };

        match status {
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(Color { a: 0.8, ..color })),
                 ..base
            },
            _ => base
        }
    }
}