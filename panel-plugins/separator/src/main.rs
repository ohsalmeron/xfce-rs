use iced::widget::{container, space};
use iced::{Background, Border, Color, Length, Theme};

pub fn main() -> iced::Result {
    iced::application(SeparatorApp::new, SeparatorApp::update, SeparatorApp::view)
        .title(SeparatorApp::title)
        .theme(SeparatorApp::theme)
        .style(SeparatorApp::style)
        .window(iced::window::Settings {
            size: iced::Size::new(8.0, 48.0),
            position: iced::window::Position::Centered,
            transparent: true,
            decorations: false,
            ..Default::default()
        })
        .run()
}

struct SeparatorApp {
    style: SeparatorStyle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SeparatorStyle {
    #[allow(dead_code)]
    /// WHY: Support for different separator appearances listed in the original XFCE spec.
    /// PLAN: Implement context menu for style switching (Ticket #SEP-01, 2024-Q1, @ohsalmeron)
    Transparent,
    Separator,
    #[allow(dead_code)]
    /// WHY: Support for different separator appearances listed in the original XFCE spec.
    /// PLAN: Implement context menu for style switching (Ticket #SEP-01, 2024-Q1, @ohsalmeron)
    Handle,
    #[allow(dead_code)]
    /// WHY: Support for different separator appearances listed in the original XFCE spec.
    /// PLAN: Implement context menu for style switching (Ticket #SEP-01, 2024-Q1, @ohsalmeron)
    Dots,
}

#[derive(Debug, Clone)]
enum Message {
    // No messages needed for a simple separator
}

impl SeparatorApp {
    fn new() -> (Self, iced::Task<Message>) {
        (
            Self {
                style: SeparatorStyle::Separator,
            },
            iced::Task::none(),
        )
    }

    fn title(&self) -> String {
        String::from("Separator")
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn style(&self, theme: &Theme) -> iced::theme::Style {
        iced::theme::Style {
            background_color: iced::Color::TRANSPARENT,
            text_color: theme.palette().text,
        }
    }

    fn update(&mut self, _message: Message) -> iced::Task<Message> {
        iced::Task::none()
    }

    fn separator_style(style: SeparatorStyle) -> impl Fn(&Theme) -> iced::widget::container::Style {
        move |_theme: &Theme| {
            match style {
                SeparatorStyle::Transparent => iced::widget::container::Style {
                    background: Some(Background::Color(Color::TRANSPARENT)),
                    ..Default::default()
                },
                SeparatorStyle::Separator => iced::widget::container::Style {
                    background: Some(Background::Color(Color::TRANSPARENT)),
                    border: Border {
                        width: 1.0,
                        radius: 0.0.into(),
                        color: Color::from_rgba(1.0, 1.0, 1.0, 0.2),
                    },
                    ..Default::default()
                },
                SeparatorStyle::Handle => iced::widget::container::Style {
                    background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.1))),
                    border: Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: Color::from_rgba(1.0, 1.0, 1.0, 0.3),
                    },
                    ..Default::default()
                },
                SeparatorStyle::Dots => iced::widget::container::Style {
                    // Placeholder for dots, could be an image or SVG
                    background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.05))),
                    ..Default::default()
                },
            }
        }
    }

    fn view(&self) -> iced::Element<'_, Message> {
        container(space())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(Self::separator_style(self.style))
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_styles_constructed() {
        let _ = SeparatorStyle::Transparent;
        let _ = SeparatorStyle::Separator;
        let _ = SeparatorStyle::Handle;
        let _ = SeparatorStyle::Dots;
    }
}
