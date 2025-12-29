use iced::widget::{
    column, container, row, text, button, slider, pick_list, space,
};
use iced::widget::checkbox;
use iced::{Alignment, Element, Length, Task, Theme};
use xfce_rs_ui::styles;
use xfce_rs_ui::colors;

use crate::settings::{PanelSettings, PanelPosition, PanelMode, AutohideBehavior};

pub struct SettingsApp {
    settings: PanelSettings,
    saved: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    SizeChanged(f32),
    IconSizeChanged(f32),
    DarkModeToggled(bool),
    PositionChanged(PanelPosition),
    PositionLockedToggled(bool),
    SpanMonitorsToggled(bool),
    AutohideChanged(AutohideBehavior),
    AutohideSizeChanged(f32),
    PopdownSpeedChanged(f32),
    ModeChanged(PanelMode),
    NRowsChanged(f32),
    LengthChanged(Option<f32>),
    LengthMaxChanged(Option<f32>),
    EnableStrutsToggled(bool),
    KeepBelowToggled(bool),
    Save,
    Cancel,
}

impl SettingsApp {
    pub fn new(settings: PanelSettings) -> (Self, Task<Message>) {
        (
            Self {
                settings,
                saved: false,
            },
            Task::none(),
        )
    }

    pub fn title(&self) -> String {
        String::from("Panel Settings")
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }

    pub fn style(&self, theme: &Theme) -> iced::theme::Style {
        iced::theme::Style {
            background_color: iced::Color::TRANSPARENT,
            text_color: theme.palette().text,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SizeChanged(val) => {
                self.settings.size = val as u32;
                self.saved = false;
                Task::none()
            }
            Message::IconSizeChanged(val) => {
                self.settings.icon_size = val as u32;
                self.saved = false;
                Task::none()
            }
            Message::DarkModeToggled(val) => {
                self.settings.dark_mode = val;
                self.saved = false;
                Task::none()
            }
            Message::PositionChanged(pos) => {
                self.settings.position = pos;
                self.saved = false;
                Task::none()
            }
            Message::PositionLockedToggled(val) => {
                self.settings.position_locked = val;
                self.saved = false;
                Task::none()
            }
            Message::SpanMonitorsToggled(val) => {
                self.settings.span_monitors = val;
                self.saved = false;
                Task::none()
            }
            Message::AutohideChanged(behavior) => {
                self.settings.autohide = behavior;
                self.saved = false;
                Task::none()
            }
            Message::AutohideSizeChanged(val) => {
                self.settings.autohide_size = val as u32;
                self.saved = false;
                Task::none()
            }
            Message::PopdownSpeedChanged(val) => {
                self.settings.popdown_speed = val as u32;
                self.saved = false;
                Task::none()
            }
            Message::ModeChanged(mode) => {
                self.settings.mode = mode;
                self.saved = false;
                Task::none()
            }
            Message::NRowsChanged(val) => {
                self.settings.nrows = val as u32;
                self.saved = false;
                Task::none()
            }
            Message::LengthChanged(val) => {
                self.settings.length = val.map(|v| v as u32);
                self.saved = false;
                Task::none()
            }
            Message::LengthMaxChanged(val) => {
                self.settings.length_max = val.map(|v| v as u32);
                self.saved = false;
                Task::none()
            }
            Message::EnableStrutsToggled(val) => {
                self.settings.enable_struts = val;
                self.saved = false;
                Task::none()
            }
            Message::KeepBelowToggled(val) => {
                self.settings.keep_below = val;
                self.saved = false;
                Task::none()
            }
            Message::Save => {
                if let Err(e) = self.settings.save() {
                    tracing::error!("Failed to save settings: {}", e);
                } else {
                    self.saved = true;
                    tracing::info!("Settings saved successfully to {:?}", PanelSettings::config_path());
                }
                Task::none()
            }
            Message::Cancel => {
                // Reload settings
                self.settings = PanelSettings::load();
                self.saved = false;
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let header = row![
            text("Panel Settings").size(24).color(colors::TEXT_PRIMARY),
            space().width(Length::Fill),
            if self.saved {
                text("✓ Saved").size(14).color(colors::ACCENT_PRIMARY)
            } else {
                text("").size(14)
            },
        ]
        .align_y(Alignment::Center)
        .padding(20);

        let appearance_section = self.view_appearance_section();
        let position_section = self.view_position_section();
        let behavior_section = self.view_behavior_section();
        let advanced_section = self.view_advanced_section();

        let buttons = row![
            button(text("Cancel").size(16))
                .on_press(Message::Cancel)
                .style(|theme, status| styles::app_card(theme, status))
                .padding(12),
            space().width(Length::Fill),
            button(text("Save").size(16))
                .on_press(Message::Save)
                .style(|theme, status| styles::app_card(theme, status))
                .padding(12),
        ]
        .padding(20);

        let content = column![
            header,
            appearance_section,
            position_section,
            behavior_section,
            advanced_section,
            buttons,
        ]
        .spacing(20)
        .padding(30);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|theme| styles::glass_base(theme))
            .into()
    }

    fn view_appearance_section(&self) -> Element<'_, Message> {
        container(
            column![
                text("Appearance").size(18).color(colors::TEXT_PRIMARY),
                row![
                    text("Panel Size:").size(14).color(colors::TEXT_SECONDARY).width(150),
                    slider(16.0..=128.0, self.settings.size as f32, Message::SizeChanged)
                        .width(200),
                    text(format!("{}px", self.settings.size)).size(12).color(colors::TEXT_SECONDARY).width(60),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                row![
                    text("Icon Size:").size(14).color(colors::TEXT_SECONDARY).width(150),
                    slider(0.0..=256.0, self.settings.icon_size as f32, Message::IconSizeChanged)
                        .width(200),
                    text(if self.settings.icon_size == 0 { "Auto".to_string() } else { format!("{}px", self.settings.icon_size) })
                        .size(12).color(colors::TEXT_SECONDARY).width(60),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                row![
                    text("Dark Mode:").size(14).color(colors::TEXT_SECONDARY).width(150),
                    checkbox(self.settings.dark_mode)
                        .label("Dark Mode")
                        .on_toggle(Message::DarkModeToggled),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                row![
                    text("Mode:").size(14).color(colors::TEXT_SECONDARY).width(150),
                    pick_list(
                        vec![PanelMode::Horizontal, PanelMode::Vertical],
                        Some(self.settings.mode),
                        Message::ModeChanged
                    )
                    .width(200),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                row![
                    text("Rows:").size(14).color(colors::TEXT_SECONDARY).width(150),
                    slider(1.0..=6.0, self.settings.nrows as f32, Message::NRowsChanged)
                        .width(200)
                        .step(1.0),
                    text(format!("{}", self.settings.nrows)).size(12).color(colors::TEXT_SECONDARY).width(60),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            ]
            .spacing(15)
        )
        .padding(20)
        .style(|theme| styles::glass_base(theme))
        .into()
    }

    fn view_position_section(&self) -> Element<'_, Message> {
        container(
            column![
                text("Position").size(18).color(colors::TEXT_PRIMARY),
                row![
                    text("Position:").size(14).color(colors::TEXT_SECONDARY).width(150),
                    pick_list(
                        vec![PanelPosition::Top, PanelPosition::Bottom, PanelPosition::Left, PanelPosition::Right],
                        Some(self.settings.position),
                        Message::PositionChanged
                    )
                    .width(200),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                row![
                    text("Lock Position:").size(14).color(colors::TEXT_SECONDARY).width(150),
                    checkbox(self.settings.position_locked)
                        .label("Lock Position")
                        .on_toggle(Message::PositionLockedToggled),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                row![
                    text("Span Monitors:").size(14).color(colors::TEXT_SECONDARY).width(150),
                    checkbox(self.settings.span_monitors)
                        .label("Span Monitors")
                        .on_toggle(Message::SpanMonitorsToggled),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            ]
            .spacing(15)
        )
        .padding(20)
        .style(|theme| styles::glass_base(theme))
        .into()
    }

    fn view_behavior_section(&self) -> Element<'_, Message> {
        container(
            column![
                text("Behavior").size(18).color(colors::TEXT_PRIMARY),
                row![
                    text("Autohide:").size(14).color(colors::TEXT_SECONDARY).width(150),
                    pick_list(
                        vec![AutohideBehavior::Never, AutohideBehavior::Intelligently, AutohideBehavior::Always],
                        Some(self.settings.autohide),
                        Message::AutohideChanged
                    )
                    .width(200),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                row![
                    text("Autohide Size:").size(14).color(colors::TEXT_SECONDARY).width(150),
                    slider(1.0..=10.0, self.settings.autohide_size as f32, Message::AutohideSizeChanged)
                        .width(200)
                        .step(1.0),
                    text(format!("{}px", self.settings.autohide_size)).size(12).color(colors::TEXT_SECONDARY).width(60),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                row![
                    text("Popdown Speed:").size(14).color(colors::TEXT_SECONDARY).width(150),
                    slider(1.0..=100.0, self.settings.popdown_speed as f32, Message::PopdownSpeedChanged)
                        .width(200),
                    text(format!("{}", self.settings.popdown_speed)).size(12).color(colors::TEXT_SECONDARY).width(60),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            ]
            .spacing(15)
        )
        .padding(20)
        .style(|theme| styles::glass_base(theme))
        .into()
    }

    fn view_advanced_section(&self) -> Element<'_, Message> {
        container(
            column![
                text("Advanced").size(18).color(colors::TEXT_PRIMARY),
                row![
                    text("Enable Struts:").size(14).color(colors::TEXT_SECONDARY).width(150),
                    checkbox(self.settings.enable_struts)
                        .label("Enable Struts")
                        .on_toggle(Message::EnableStrutsToggled),
                    text("(Reserve screen space)").size(12).color(colors::TEXT_SECONDARY),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                row![
                    text("Keep Below:").size(14).color(colors::TEXT_SECONDARY).width(150),
                    checkbox(self.settings.keep_below)
                        .label("Keep Below")
                        .on_toggle(Message::KeepBelowToggled),
                    text("(Keep panel below other windows)").size(12).color(colors::TEXT_SECONDARY),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            ]
            .spacing(15)
        )
        .padding(20)
        .style(|theme| styles::glass_base(theme))
        .into()
    }

    pub fn get_settings(&self) -> &PanelSettings {
        &self.settings
    }
}
