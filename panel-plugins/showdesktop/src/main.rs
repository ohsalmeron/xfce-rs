use iced::widget::{button, container, text};
use iced::{Alignment, Element, Length, Task, Theme};
use xfce_rs_ui::styles;
use tracing::{info, warn};
use std::process::Command;

pub fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    
    info!("Show Desktop plugin starting");
    
    iced::application(ShowDesktopApp::new, ShowDesktopApp::update, ShowDesktopApp::view)
        .title(ShowDesktopApp::title)
        .theme(ShowDesktopApp::theme)
        .style(ShowDesktopApp::style)
        .window(iced::window::Settings {
            size: iced::Size::new(48.0, 48.0),
            position: iced::window::Position::Centered,
            transparent: true,
            decorations: false,
            ..Default::default()
        })
        .run()
}

struct ShowDesktopApp {
    is_shown: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Toggle,
}

impl ShowDesktopApp {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                is_shown: false,
            },
            Task::none(),
        )
    }

    fn title(&self) -> String {
        String::from("Show Desktop")
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

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Toggle => {
                self.is_shown = !self.is_shown;
                self.toggle_show_desktop();
                Task::none()
            }
        }
    }

    fn toggle_show_desktop(&self) {
        // Try to use xfce4-window-manager or wmctrl to show/hide desktop
        // This is a simplified implementation - in a real scenario, we'd use
        // proper window manager APIs
        
        // Try wmctrl first (common on X11)
        let result = Command::new("wmctrl")
            .arg("-k")
            .arg(if self.is_shown { "on" } else { "off" })
            .output();
        
        if result.is_err() {
            // Fallback: try xdotool or other methods
            warn!("Could not toggle show desktop - wmctrl not available");
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let icon = if self.is_shown { "📋" } else { "🖥️" };
        
        let button_widget = button(
            container(
                text(icon).size(24)
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
        )
        .on_press(Message::Toggle)
        .style(|theme, status| styles::app_card(theme, status))
        .width(Length::Fill)
        .height(Length::Fill);

        container(button_widget)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(4)
            .style(|theme| styles::glass_base(theme))
            .into()
    }
}
