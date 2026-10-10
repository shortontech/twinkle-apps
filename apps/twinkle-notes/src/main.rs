use std::fs;

use twinkle_ui::prelude::*;
use twinkle_ui::{VerticalScroll, ViewContext};

#[derive(Default)]
struct Notes {
    text: String,
    path: String,
    scroll: f32,
    status: String,
}

#[derive(Clone)]
enum Message {
    EditText(String),
    EditPath(String),
    New,
    Open,
    Save,
    Noop,
    Scroll(f32),
}

impl Application for Notes {
    type Message = Message;

    fn update(&mut self, message: Message) {
        match message {
            Message::EditText(text) => {
                self.text = text;
                self.status = "Unsaved changes".into();
            }
            Message::EditPath(path) => self.path = path,
            Message::New => {
                self.text.clear();
                self.path.clear();
                self.status = "New document".into();
            }
            Message::Open => match fs::read_to_string(&self.path) {
                Ok(text) => {
                    self.text = text;
                    self.status = format!("Opened {}", self.path);
                }
                Err(error) => self.status = format!("Could not open {}: {error}", self.path),
            },
            Message::Save => match fs::write(&self.path, &self.text) {
                Ok(()) => self.status = format!("Saved {}", self.path),
                Err(error) => self.status = format!("Could not save {}: {error}", self.path),
            },
            Message::Noop => {}
            Message::Scroll(offset) => self.scroll = offset,
        }
    }

    fn view(&self, _context: ViewContext) -> impl View<Message> {
        let line_count = self.text.lines().count().max(1);
        let character_count = self.text.chars().count();

        Column::new()
            .id(id!(notes_app))
            .gap(0.0)
            .padding(Insets::all(0.0))
            .background(0xfaf9f6)
            .foreground(0x202020)
            .fill_width()
            .fill_height()
            .child(
                Row::new()
                    .gap(8.0)
                    .padding(Insets::all(10.0))
                    .background(0xf0efed)
                    .child(Text::new("File path"))
                    .child(
                        TextField::on_change_with_placeholder(
                            &self.path,
                            "notes.txt",
                            Message::EditPath,
                        )
                        .id(id!(file_path))
                        .accessibility_label("File path")
                        .background(0xffffff)
                        .color(0x202020)
                        .grow(1.0),
                    )
                    .child(Button::new(Message::New, "New"))
                    .child(Button::new(Message::Open, "Open"))
                    .child(Button::new(Message::Save, "Save")),
            )
            .child(
                VerticalScroll::new(Message::Noop, self.scroll)
                    .id(id!(editor_scroll))
                    .grow(1.0)
                    .on_scroll(Message::Scroll)
                    .controlled(true)
                    .child(
                        TextField::on_change(&self.text, Message::EditText)
                            .id(id!(document_text))
                            .accessibility_label("Document text")
                            .background(0xffffff)
                            .color(0x202020)
                            .width_length(Length::Fill)
                            .wrap(true)
                            .min_height(350.0)
                            .grow(1.0),
                    ),
            )
            .child(Text::new(format!(
                "{line_count} lines · {character_count} characters · {}",
                if self.status.is_empty() {
                    "Ready"
                } else {
                    &self.status
                }
            )))
    }

    fn title(&self) -> &str {
        "Twinkle Notes"
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run(Notes::default())
}
