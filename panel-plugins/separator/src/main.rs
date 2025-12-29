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

#[derive(Debug, Clone, Copy)]
enum SeparatorStyle {
    Transparent,
    Separator,
    Handle,
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

    fn separator_style(_theme: &Theme) -> iced::widget::container::Style {
        iced::widget::container::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            border: Border {
                width: 1.0,
                radius: 0.0.into(),
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.2),
            },
            ..Default::default()
        }
    }

    fn view(&self) -> iced::Element<'_, Message> {
        // Draw separator line - simple vertical line
        container(space())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(Self::separator_style)
            .into()
    }
}
