use iced::widget::{column, container, row, text, text_input, scrollable, button, image, svg, stack, space};
use iced::{Alignment, Element, Length, Task, Theme, Color, window};
use freedesktop_desktop_entry::{DesktopEntry, Iter as DesktopIter};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use linicon;
use xfce_rs_ui::styles;
use xfce_rs_ui::colors;

pub fn main() -> iced::Result {
    iced::application(AppFinder::new, AppFinder::update, AppFinder::view)
        .title(AppFinder::title)
        .theme(AppFinder::theme)
        .style(AppFinder::style)
        .window(iced::window::Settings {
            size: iced::Size::new(700.0, 500.0),
            position: iced::window::Position::Centered,
            transparent: true,
            decorations: false,
            ..Default::default()
        })
        .run()
}

#[derive(Default)]
struct AppFinder {
    query: String,
    apps: Vec<AppEntry>,
    filtered_apps: Vec<AppEntry>,
    maximized: bool,
}

#[derive(Debug, Clone)]
enum Message {
    QueryChanged(String),
    LaunchApp(String),
    WindowDragged,
    Minimize,
    Maximize,
    Close,
}

/// Represents the source of an icon to render differently in the view
#[derive(Debug, Clone)]
enum IconSource {
    Svg(PathBuf),
    Raster(PathBuf),
}

#[derive(Debug, Clone)]
struct AppEntry {
    name: String,
    exec: String,
    _id: String,
    icon: Option<IconSource>,
}

impl AppFinder {
    fn new() -> (Self, Task<Message>) {
        let apps = scan_desktop_entries();
        let filtered_apps = apps.clone();
        (
            Self {
                query: String::new(),
                apps,
                filtered_apps,
                maximized: false,
            },
            Task::none(),
        )
    }

    fn title(&self) -> String {
        String::from("App Finder")
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
            Message::QueryChanged(new_query) => {
                self.query = new_query;
                if self.query.is_empty() {
                    self.filtered_apps = self.apps.clone();
                } else {
                    let matcher = SkimMatcherV2::default();
                    let mut scored: Vec<(i64, AppEntry)> = self.apps
                        .iter()
                        .filter_map(|app| {
                            matcher.fuzzy_match(&app.name, &self.query)
                                .map(|score| (score, app.clone()))
                        })
                        .collect();
                    scored.sort_by(|a, b| b.0.cmp(&a.0));
                    self.filtered_apps = scored.into_iter().map(|(_, app)| app).collect();
                }
                Task::none()
            }
            Message::LaunchApp(exec) => {
                let cleaned = exec
                    .replace("%f", "").replace("%F", "")
                    .replace("%u", "").replace("%U", "")
                    .trim().to_string();
                
                let parts: Vec<&str> = cleaned.split_whitespace().collect();
                if !parts.is_empty() {
                    let _ = StdCommand::new(parts[0])
                        .args(&parts[1..])
                        .spawn();
                }
                std::process::exit(0);
            }
            Message::WindowDragged => {
                 window::latest().and_then(|id| window::drag(id))
            }
            Message::Minimize => {
                window::latest().and_then(|id| window::minimize(id, true))
            }
            Message::Maximize => {
                self.maximized = !self.maximized;
                let maximized = self.maximized;
                window::latest().and_then(move |id| window::maximize(id, maximized))
            }
            Message::Close => {
                window::latest().and_then(|id| window::close(id))
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let header = row![
             row![
                button(space().width(12).height(12))
                    .on_press(Message::Close)
                    .style(|theme, status| styles::window_control(theme, status, colors::CONTROL_CLOSE))
                    .width(12).height(12),
                button(space().width(12).height(12))
                     .on_press(Message::Minimize)
                     .style(|theme, status| styles::window_control(theme, status, colors::CONTROL_MIN))
                     .width(12).height(12),
                button(space().width(12).height(12))
                     .on_press(Message::Maximize)
                     .style(|theme, status| styles::window_control(theme, status, colors::CONTROL_MAX))
                     .width(12).height(12),
            ]
            .spacing(8)
            .padding(10),
            
            // Drag handle
            // Title (Static text, clicks fall through to global MouseArea)
            container(text("XFCE.rs App Finder")
                    .size(14)
                    .color(colors::TEXT_SECONDARY)
                    .width(Length::Fill)
                    .align_x(Alignment::Center))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_y(Alignment::Center)
        ]
        .height(40)
        .align_y(Alignment::Center);


        let input = text_input("Search applications...", &self.query)
            .on_input(Message::QueryChanged)
            .padding(15)
            .size(20)
            .style(|theme, status| styles::search_input(theme, status));

        let content = self.filtered_apps.iter().fold(
            column![].spacing(10).width(Length::Fill),
            |column, app| {
                let icon_widget: Element<Message> = match &app.icon {
                    Some(IconSource::Svg(path)) => {
                        svg(svg::Handle::from_path(path))
                            .width(32)
                            .height(32)
                            .into()
                    }
                    Some(IconSource::Raster(path)) => {
                        image(path).width(32).height(32).into()
                    }
                    None => {
                        text("📦").size(32).into()
                    }
                };

                column.push(
                    button(
                        row![
                            icon_widget,
                            text(&app.name).size(18).color(Color::WHITE),
                        ]
                        .spacing(15)
                        .align_y(Alignment::Center),
                    )
                    .on_press(Message::LaunchApp(app.exec.clone()))
                    .width(Length::Fill)
                    .padding(12)
                    .style(|theme, status| styles::app_card(theme, status)),
                )
            },
        );

        let main_content = column![
            header,
            input,
            scrollable(content).height(Length::Fill)
        ]
        .spacing(10) // Reduced spacing since header is there
        .padding(20);

        stack![
            // Layer 1: Base Glass
            container(space()).width(Length::Fill).height(Length::Fill).style(|theme| styles::glass_base(theme)),
            // Layer 2: Edge Highlights (Boxed Gloss)
            container(space()).width(Length::Fill).height(Length::Fill).style(|theme| styles::glass_highlight_top(theme)),
            container(space()).width(Length::Fill).height(Length::Fill).style(|theme| styles::glass_highlight_bottom(theme)),
            container(space()).width(Length::Fill).height(Length::Fill).style(|theme| styles::glass_highlight_left(theme)),
            container(space()).width(Length::Fill).height(Length::Fill).style(|theme| styles::glass_highlight_right(theme)),

            // Layer 3: Global Drag Listener (Transparent Button) - NOW ON TOP OF GLASS
            // Layer 3: Global Drag Listener (Mouse Area) - NOW ON TOP OF GLASS
            iced::widget::mouse_area(container(space()).width(Length::Fill).height(Length::Fill))
                .on_press(Message::WindowDragged),

            // Layer 4: Content (Blocks Drag Layer where content exists)
            container(main_content).width(Length::Fill).height(Length::Fill)
        ]
        .into()
    }
}

/// Resolves an icon source from a .desktop Icon key.
/// Follows the xfce4-panel fallback strategy:
/// 1. Absolute path -> use directly
/// 2. Icon theme lookup
/// 3. Strip extension and try icon theme again
/// 4. Look in /usr/share/pixmaps
fn resolve_icon(icon_key: &str) -> Option<IconSource> {
    let path = Path::new(icon_key);

    // 1. Check if it's an absolute path
    if path.is_absolute() && path.exists() {
        return path_to_icon_source(path);
    }

    // 2. Try linicon (icon theme lookup)
    if let Some(found) = linicon::lookup_icon(icon_key)
        .with_size(32)
        .next()
        .and_then(|r| r.ok())
    {
        return path_to_icon_source(&found.path);
    }

    // 3. Strip extension and try icon theme again (e.g., "app.png" -> "app")
    let name_without_ext = path.file_stem().and_then(|s| s.to_str()).unwrap_or(icon_key);
    if name_without_ext != icon_key {
        if let Some(found) = linicon::lookup_icon(name_without_ext)
            .with_size(32)
            .next()
            .and_then(|r| r.ok())
        {
            return path_to_icon_source(&found.path);
        }
    }

    // 4. Look in /usr/share/pixmaps
    for ext in &["svg", "png", "xpm"] {
        let pixmap_path = PathBuf::from(format!("/usr/share/pixmaps/{}.{}", icon_key, ext));
        if pixmap_path.exists() {
            return path_to_icon_source(&pixmap_path);
        }
    }

    None
}

fn path_to_icon_source(path: &Path) -> Option<IconSource> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext.to_lowercase().as_str() {
        "svg" => Some(IconSource::Svg(path.to_path_buf())),
        "png" | "jpg" | "jpeg" | "xpm" => Some(IconSource::Raster(path.to_path_buf())),
        _ => None,
    }
}


fn scan_desktop_entries() -> Vec<AppEntry> {
    let mut entries = Vec::new();
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/share:/usr/local/share".to_string());
    
    let mut search_paths: Vec<PathBuf> = data_dirs
        .split(':')
        .map(|p| PathBuf::from(p).join("applications"))
        .collect();

    if let Some(home) = dirs::home_dir() {
        search_paths.push(home.join(".local/share/applications"));
    }

    let locales: &[&str] = &["en_US", "en"];

    for entry_path in DesktopIter::new(search_paths.into_iter()) {
        if let Ok(bytes) = std::fs::read_to_string(&entry_path) {
            if let Ok(desktop) = DesktopEntry::from_str(&entry_path, &bytes, Some(locales)) {
                if desktop.no_display() || desktop.hidden() {
                    continue;
                }

                let exec = match desktop.exec() {
                    Some(e) => e.to_string(),
                    None => continue,
                };

                let id = entry_path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let name = desktop.name(locales)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| id.clone());
                
                let icon = desktop.icon().and_then(resolve_icon);

                entries.push(AppEntry { name, exec, _id: id, icon });
            }
        }
    }

    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    entries
}
