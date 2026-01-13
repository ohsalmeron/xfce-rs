use iced::widget::{container, text};
use iced::{Alignment, Element, Length};
use xfce_rs_ui::styles;
use xfce_rs_ui::colors;

use crate::plugin_manager::PluginInfo;

pub struct PluginSlot {
    plugin: PluginInfo,
    is_running: bool,
    position: Option<PluginPosition>,
}

#[derive(Debug, Clone)]
pub struct PluginPosition {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl PluginSlot {
    pub fn new(plugin: PluginInfo) -> Self {
        Self {
            plugin,
            is_running: false,
            position: None,
        }
    }

    pub fn set_position(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.position = Some(PluginPosition { x, y, width, height });
    }

    pub fn position(&self) -> Option<&PluginPosition> {
        self.position.as_ref()
    }

    pub fn is_embedded(&self) -> bool {
        !self.plugin.detached && self.position.is_some()
    }

    pub fn view(&self) -> Element<'_, crate::Message> {
        // For now, show plugin name and status
        // In embedded mode, we'd embed the plugin window here
        // In detached mode, we just show a status indicator
        
        let content = if self.plugin.detached {
            // Detached mode: show status indicator
            container(
                text(&self.plugin.name)
                    .size(12)
                    .color(if self.is_running { colors::ACCENT_PRIMARY } else { colors::TEXT_SECONDARY })
            )
            .width(Length::Shrink)
            .height(Length::Fill)
            .padding(8)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
        } else {
            // Embedded mode: placeholder for embedded plugin
            // In a real implementation, we'd embed the plugin window here
            container(
                text(&self.plugin.description)
                    .size(12)
                    .color(colors::TEXT_PRIMARY)
            )
            .width(Length::Shrink)
            .height(Length::Fill)
            .padding(8)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(|theme| styles::glass_base(theme))
        };

        content.into()
    }

    pub fn plugin_name(&self) -> &str {
        &self.plugin.name
    }

    pub fn plugin_info(&self) -> &PluginInfo {
        &self.plugin
    }

    pub fn set_running(&mut self, running: bool) {
        self.is_running = running;
    }
}
