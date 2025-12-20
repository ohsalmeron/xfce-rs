use iced::widget::{column, container, row, text, text_input, scrollable, button};
use iced::{Alignment, Application, Command, Element, Length, Settings, Theme};
use freedesktop_desktop_entry::{DesktopEntry, Iter as DesktopIter};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::path::PathBuf;
use std::process::Command as StdCommand;

pub fn main() -> iced::Result {
    AppFinder::run(Settings {
        window: iced::window::Settings {
            size: iced::Size::new(700.0, 500.0),
            position: iced::window::Position::Centered,
            decorations: true,
            transparent: false,
            ..Default::default()
        },
        ..Default::default()
    })
}

struct AppFinder {
    query: String,
    apps: Vec<AppEntry>,
    filtered_apps: Vec<AppEntry>,
}

#[derive(Debug, Clone)]
enum Message {
    QueryChanged(String),
    LaunchApp(String),
}

#[derive(Debug, Clone)]
struct AppEntry {
    name: String,
    exec: String,
    _id: String,
}

impl Application for AppFinder {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let apps = scan_desktop_entries();
        let filtered_apps = apps.clone();
        (
            Self {
                query: String::new(),
                apps,
                filtered_apps,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("XFCE.rs App Finder")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
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
                Command::none()
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
                iced::window::close(iced::window::Id::MAIN)
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let input = text_input("Search applications...", &self.query)
            .on_input(Message::QueryChanged)
            .padding(15)
            .size(20);

        let content = self.filtered_apps.iter().fold(
            column![].spacing(10).width(Length::Fill),
            |column, app| {
                column.push(
                    button(
                        row![
                            text(&app.name).size(18),
                        ]
                        .spacing(10)
                        .align_items(Alignment::Center),
                    )
                    .on_press(Message::LaunchApp(app.exec.clone()))
                    .width(Length::Fill)
                    .padding(12)
                    .style(iced::theme::Button::Secondary),
                )
            },
        );

        container(
            column![
                input,
                scrollable(content).height(Length::Fill)
            ]
            .spacing(20)
            .padding(20)
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x()
        .into()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
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

                entries.push(AppEntry { name, exec, _id: id });
            }
        }
    }

    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    entries
}
