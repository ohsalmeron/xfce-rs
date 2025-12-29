use iced::widget::{column, container, text};
use iced::{Alignment, Element, Length, Task, Theme, Subscription};
use iced::time;
use chrono::{DateTime, Local};
use std::time::Duration;
use xfce_rs_ui::styles;
use xfce_rs_ui::colors;
use tracing::info;

pub fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    
    info!("Clock plugin starting");
    
    iced::application(ClockApp::new, ClockApp::update, ClockApp::view)
        .title(ClockApp::title)
        .theme(ClockApp::theme)
        .style(ClockApp::style)
        .subscription(ClockApp::subscription)
        .window(iced::window::Settings {
            size: iced::Size::new(200.0, 48.0),
            position: iced::window::Position::Centered,
            transparent: true,
            decorations: false,
            ..Default::default()
        })
        .run()
}

struct ClockApp {
    current_time: DateTime<Local>,
    format: String,
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
}

impl ClockApp {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                current_time: Local::now(),
                format: "%H:%M".to_string(),
            },
            Task::none(),
        )
    }

    fn title(&self) -> String {
        String::from("Clock")
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

    fn subscription(&self) -> Subscription<Message> {
        time::every(Duration::from_secs(1)).map(|_| Message::Tick)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                self.current_time = Local::now();
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let time_str = self.current_time.format(&self.format).to_string();
        let date_str = self.current_time.format("%A, %B %d").to_string();
        
        let content = column![
            text(time_str)
                .size(18)
                .color(colors::TEXT_PRIMARY),
            text(date_str)
                .size(12)
                .color(colors::TEXT_SECONDARY),
        ]
        .spacing(4)
        .align_x(Alignment::Center);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(8)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(|theme| styles::glass_base(theme))
            .into()
    }
}
