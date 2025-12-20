use iced::{Color, Theme, Background};
use iced::widget::button;

/// Design System Constants
pub mod colors {
    use iced::Color;
    
    pub const BG_PRIMARY: Color = Color::from_rgb(0.06, 0.06, 0.08); // #0f0f14
    pub const BG_SECONDARY: Color = Color::from_rgb(0.09, 0.09, 0.11); // #16161d
    pub const BG_CARD: Color = Color::from_rgb(0.11, 0.11, 0.15); // #1c1c26
    
    pub const ACCENT_PRIMARY: Color = Color::from_rgb(0.49, 0.36, 1.0); // #7c5cff
    pub const ACCENT_SECONDARY: Color = Color::from_rgb(0.37, 0.62, 1.0); // #5e9eff
    
    pub const TEXT_PRIMARY: Color = Color::from_rgb(0.94, 0.94, 0.96); // #f0f0f5
    pub const TEXT_SECONDARY: Color = Color::from_rgb(0.53, 0.53, 0.63); // #8888a0
}

/// Custom Styles for Iced Widgets
pub mod styles {
    use iced::widget::button;
    use iced::{Background, Color, Vector, border};
    use super::colors;

    pub struct AppCard;

    impl button::StyleSheet for AppCard {
        type Style = iced::Theme;

        fn active(&self, _style: &Self::Style) -> button::Appearance {
            button::Appearance {
                background: Some(Background::Color(colors::BG_CARD)),
                border: border::secondary(colors::BG_CARD, 12.0),
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.2),
                    offset: Vector::new(0.0, 4.0),
                    blur_radius: 12.0,
                },
                ..Default::default()
            }
        }

        fn hovered(&self, style: &Self::Style) -> button::Appearance {
            let active = self.active(style);
            button::Appearance {
                background: Some(Background::Color(Color::from_rgb(0.15, 0.15, 0.2))),
                border: border::secondary(colors::ACCENT_PRIMARY, 12.0),
                shadow: iced::Shadow {
                    offset: Vector::new(0.0, 6.0),
                    blur_radius: 16.0,
                    ..active.shadow
                },
                ..active
            }
        }

        fn pressed(&self, style: &Self::Style) -> button::Appearance {
            button::Appearance {
                shadow: iced::Shadow::default(),
                ..self.hovered(style)
            }
        }
    }
}