mod window_tracker;

use iced::widget::{container, row, button, text};
use iced::{Alignment, Element, Length, Task, Theme, Subscription};
use iced::time;
use std::time::Duration;
use xfce_rs_ui::styles;
use xfce_rs_ui::colors;
use tracing::{info, warn};
use xfce_rs_utils::x11_window_props;
use clap::Parser;

use window_tracker::{WindowTracker, WindowInfo};

#[derive(Parser, Debug)]
#[command(name = "xfce-rs-tasklist")]
#[command(about = "Window buttons plugin for XFCE.rs panel")]
struct Args {
    /// X position for panel embedding
    #[arg(long = "panel-x")]
    panel_x: Option<f32>,
    
    /// Y position for panel embedding
    #[arg(long = "panel-y")]
    panel_y: Option<f32>,
    
    /// Width for panel embedding
    #[arg(long = "panel-width")]
    panel_width: Option<f32>,
    
    /// Height for panel embedding
    #[arg(long = "panel-height")]
    panel_height: Option<f32>,
}

pub fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    
    let args = Args::parse();
    
    // Determine window position and size
    let (x, y, width, height) = if let (Some(x), Some(y), Some(w), Some(h)) = 
        (args.panel_x, args.panel_y, args.panel_width, args.panel_height) {
        // Embedded mode: use provided position
        (x, y, w, h)
    } else {
        // Standalone mode: centered window for testing
        (400.0, 300.0, 600.0, 48.0)
    };
    
    info!("Tasklist plugin starting (embedded: {}, pos: {},{} size: {}x{})", 
        args.panel_x.is_some(), x, y, width, height);
    
    iced::application(
        TasklistApp::new,
        TasklistApp::update,
        TasklistApp::view,
    )
    .title(TasklistApp::title)
    .theme(TasklistApp::theme)
    .style(TasklistApp::style)
    .subscription(TasklistApp::subscription)
    .window(iced::window::Settings {
        size: iced::Size::new(width, height),
        position: iced::window::Position::Specific(iced::Point::new(x, y)),
        transparent: true,
        decorations: false,
        resizable: false,
        ..Default::default()
    })
    .run()
}

struct TasklistApp {
    windows: Vec<WindowInfo>,
    tracker: Option<WindowTracker>,
    error: Option<String>,
}

#[derive(Debug, Clone)]
enum Message {
    Refresh,
    WindowClicked(u32),
    Error(String),
}

impl TasklistApp {
    fn new() -> (Self, Task<Message>) {
        info!("Initializing tasklist plugin");
        
        // Try to initialize window tracker
        let tracker = match WindowTracker::new() {
            Ok(t) => {
                info!("Window tracker initialized successfully");
                Some(t)
            }
            Err(e) => {
                warn!("Failed to initialize window tracker: {}", e);
                None
            }
        };
        
        let app = Self {
            windows: Vec::new(),
            tracker,
            error: None,
        };
        
        (
            app,
            Task::batch(vec![
                Task::perform(async move {
                    // Small delay to let window initialize
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    Message::Refresh
                }, |_| Message::Refresh),
                Task::perform(async move {
                    // Set X11 window properties after window is created
                    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                    if let Err(e) = x11_window_props::set_plugin_window_properties("Window Buttons") {
                        warn!("Failed to set plugin window properties: {}", e);
                    }
                    Message::Refresh
                }, |_| Message::Refresh),
            ]),
        )
    }

    fn title(&self) -> String {
        String::from("Window Buttons")
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
        // Poll for window changes every 1.5 seconds
        time::every(Duration::from_millis(1500)).map(|_| Message::Refresh)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Refresh => {
                if let Some(ref tracker) = self.tracker {
                    match tracker.get_windows() {
                        Ok(windows) => {
                            self.windows = windows;
                            self.error = None;
                        }
                        Err(e) => {
                            warn!("Failed to get windows: {}", e);
                            self.error = Some(format!("Failed to get windows: {}", e));
                        }
                    }
                } else {
                    self.error = Some("Window tracker not initialized".to_string());
                }
                Task::none()
            }
            Message::WindowClicked(window_id) => {
                if let Some(ref tracker) = self.tracker {
                    if let Err(e) = tracker.activate_window(window_id) {
                        warn!("Failed to activate window {}: {}", window_id, e);
                        self.error = Some(format!("Failed to activate window: {}", e));
                    } else {
                        info!("Activated window {}", window_id);
                        // Refresh to update active state
                        return Task::perform(async {
                            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                            Message::Refresh
                        }, |_| Message::Refresh);
                    }
                }
                Task::none()
            }
            Message::Error(msg) => {
                self.error = Some(msg);
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        if let Some(ref error) = self.error {
            return container(
                text(error)
                    .size(12)
                    .color(colors::TEXT_SECONDARY)
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(8)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into();
        }

        if self.windows.is_empty() {
            return container(
                text("No windows")
                    .size(12)
                    .color(colors::TEXT_SECONDARY)
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(8)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into();
        }

        // Create buttons for each window
        let window_buttons: Vec<Element<'_, Message>> = self.windows.iter()
            .map(|win| {
                let button_text = if win.name.len() > 30 {
                    format!("{}...", &win.name[..27])
                } else {
                    win.name.clone()
                };
                
                let button_style = if win.is_active {
                    colors::ACCENT_PRIMARY
                } else if win.is_minimized {
                    colors::TEXT_SECONDARY
                } else {
                    colors::TEXT_PRIMARY
                };
                
                button(
                    container(
                        text(button_text)
                            .size(12)
                            .color(button_style)
                    )
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(8)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                )
                .on_press(Message::WindowClicked(win.window_id))
                .style(|theme, status| styles::app_card(theme, status))
                .width(Length::Shrink)
                .height(Length::Fill)
                .into()
            })
            .collect();

        let content = row(window_buttons)
            .spacing(4)
            .align_y(Alignment::Center)
            .padding(4);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|theme| styles::glass_base(theme))
            .into()
    }
}
