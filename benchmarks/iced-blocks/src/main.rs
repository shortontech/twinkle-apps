use std::time::Duration;

use iced::keyboard::{self, Key, key::Named};
use iced::widget::{Column, Row, button, column, container, row, text};
use iced::{Background, Border, Color, Element, Length, Subscription, Theme};

const W: usize = 10;
const H: usize = 20;
const SHAPES: [[(i32, i32); 4]; 7] = [
    [(0, 0), (1, 0), (0, 1), (1, 1)],
    [(0, 0), (1, 0), (2, 0), (3, 0)],
    [(0, 0), (1, 0), (2, 0), (1, 1)],
    [(0, 0), (0, 1), (1, 1), (2, 1)],
    [(2, 0), (0, 1), (1, 1), (2, 1)],
    [(1, 0), (2, 0), (0, 1), (1, 1)],
    [(0, 0), (1, 0), (1, 1), (2, 1)],
];
const COLORS: [u32; 7] = [
    0xf4d35e, 0x4cc9f0, 0xb388ff, 0x80ed99, 0xff9f1c, 0xff5d8f, 0x72efdd,
];

fn rgb(value: u32) -> Color {
    Color::from_rgb8((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

#[derive(Clone)]
struct Game {
    cells: Vec<Option<u8>>,
    shape: u8,
    x: i32,
    y: i32,
    rotation: u8,
    score: u32,
    lines: u32,
    next: u8,
    bag: [u8; 7],
    bag_index: usize,
    random_state: u32,
    paused: bool,
    over: bool,
}

impl Default for Game {
    fn default() -> Self {
        Self {
            cells: vec![None; W * H],
            shape: 1,
            x: 3,
            y: 0,
            rotation: 0,
            score: 0,
            lines: 0,
            next: 2,
            bag: [6, 5, 0, 3, 4, 1, 2],
            bag_index: 0,
            random_state: 0x6d2b_79f5,
            paused: false,
            over: false,
        }
    }
}

impl Game {
    fn draw_piece(&mut self) -> u8 {
        if self.bag_index == self.bag.len() {
            self.bag = [0, 1, 2, 3, 4, 5, 6];
            for index in (1..self.bag.len()).rev() {
                self.random_state ^= self.random_state << 13;
                self.random_state ^= self.random_state >> 17;
                self.random_state ^= self.random_state << 5;
                let other = self.random_state as usize % (index + 1);
                self.bag.swap(index, other);
            }
            self.bag_index = 0;
        }
        let piece = self.bag[self.bag_index];
        self.bag_index += 1;
        piece
    }

    fn shape_coords(shape: u8, rotation: u8) -> [(i32, i32); 4] {
        if shape == 0 {
            return SHAPES[0];
        }
        if shape == 1 {
            return match rotation % 4 {
                0 => [(0, 1), (1, 1), (2, 1), (3, 1)],
                1 => [(2, 0), (2, 1), (2, 2), (2, 3)],
                2 => [(0, 2), (1, 2), (2, 2), (3, 2)],
                _ => [(1, 0), (1, 1), (1, 2), (1, 3)],
            };
        }
        SHAPES[shape as usize].map(|(x, y)| match rotation % 4 {
            0 => (x, y),
            1 => (2 - y, x),
            2 => (2 - x, 2 - y),
            _ => (y, 2 - x),
        })
    }

    fn coords(&self) -> [(i32, i32); 4] {
        Self::shape_coords(self.shape, self.rotation)
    }

    fn fits(&self, x: i32, y: i32, rotation: u8) -> bool {
        Self::shape_coords(self.shape, rotation)
            .iter()
            .all(|&(px, py)| {
                let (cx, cy) = (x + px, y + py);
                cx >= 0
                    && cx < W as i32
                    && cy < H as i32
                    && (cy < 0 || self.cells[cy as usize * W + cx as usize].is_none())
            })
    }

    fn lock_piece(&mut self) {
        for (px, py) in self.coords() {
            let (x, y) = (self.x + px, self.y + py);
            if y >= 0 && y < H as i32 && x >= 0 && x < W as i32 {
                self.cells[y as usize * W + x as usize] = Some(self.shape);
            }
        }
        let mut kept = Vec::with_capacity(H * W);
        let mut cleared = 0;
        for board_row in self.cells.as_chunks::<W>().0 {
            if board_row.iter().all(Option::is_some) {
                cleared += 1;
            } else {
                kept.extend_from_slice(board_row);
            }
        }
        for _ in 0..cleared {
            kept.splice(0..0, std::iter::repeat_n(None, W));
        }
        self.cells = kept;
        self.lines += cleared as u32;
        self.score += [0, 100, 300, 500, 800][cleared.min(4)] * self.level();
        self.shape = self.next;
        self.next = self.draw_piece();
        self.x = 3;
        self.y = 0;
        self.rotation = 0;
        self.over = !self.fits(self.x, self.y, self.rotation);
    }

    fn level(&self) -> u32 {
        1 + self.lines / 10
    }

    fn gravity_interval(&self) -> Duration {
        Duration::from_millis(
            700_u64
                .saturating_sub(u64::from(self.level() - 1) * 55)
                .max(90),
        )
    }

    fn gravity_tick(&mut self) {
        if self.fits(self.x, self.y + 1, self.rotation) {
            self.y += 1;
        } else {
            self.lock_piece();
        }
    }

    fn act(&mut self, action: &str) {
        if action == "restart" {
            *self = Self::default();
            return;
        }
        if action == "toggle" {
            if !self.over {
                self.paused = !self.paused;
            }
            return;
        }
        if self.paused || self.over {
            return;
        }
        match action {
            "left" if self.fits(self.x - 1, self.y, self.rotation) => self.x -= 1,
            "right" if self.fits(self.x + 1, self.y, self.rotation) => self.x += 1,
            "rotate" => {
                if self.shape == 0 {
                    return;
                }
                let next = self.rotation.wrapping_add(1);
                if let Some(kick) = [0, -1, 1, -2, 2]
                    .into_iter()
                    .find(|kick| self.fits(self.x + kick, self.y, next))
                {
                    self.x += kick;
                    self.rotation = next;
                }
            }
            "down" => {
                if self.fits(self.x, self.y + 1, self.rotation) {
                    self.y += 1;
                    self.score += 1;
                } else {
                    self.lock_piece();
                }
            }
            "drop" => {
                let mut distance = 0;
                while self.fits(self.x, self.y + 1, self.rotation) {
                    self.y += 1;
                    distance += 1;
                }
                self.score += distance * 2;
                self.lock_piece();
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
    KeyPressed(Key, bool),
    Continue,
    Restart,
}

#[derive(Default)]
struct IcedBlocks {
    game: Game,
    menu_open: bool,
}

impl IcedBlocks {
    fn board(&self) -> Vec<Option<u8>> {
        let mut board = self.game.cells.clone();
        for (px, py) in self.game.coords() {
            let (x, y) = (self.game.x + px, self.game.y + py);
            if y >= 0 && y < H as i32 && x >= 0 && x < W as i32 {
                board[y as usize * W + x as usize] = Some(self.game.shape);
            }
        }
        board
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::Tick => self.game.gravity_tick(),
            Message::Continue => self.menu_open = false,
            Message::Restart => {
                self.game.act("restart");
                self.menu_open = false;
            }
            Message::KeyPressed(key, repeat) => match key.as_ref() {
                Key::Named(Named::Escape) if !repeat && !self.game.over => {
                    self.menu_open = !self.menu_open;
                }
                _ if self.menu_open => {}
                Key::Named(Named::ArrowUp) if !repeat => self.game.act("rotate"),
                Key::Named(Named::ArrowDown) => self.game.act("down"),
                Key::Named(Named::ArrowLeft) => self.game.act("left"),
                Key::Named(Named::ArrowRight) => self.game.act("right"),
                Key::Named(Named::Enter) if !repeat => self.game.act("drop"),
                Key::Character("r") if !repeat => self.game.act("restart"),
                _ => {}
            },
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let keyboard = keyboard::listen().filter_map(|event| match event {
            keyboard::Event::KeyPressed { key, repeat, .. } => {
                Some(Message::KeyPressed(key, repeat))
            }
            _ => None,
        });
        let gravity = if self.game.paused || self.game.over || self.menu_open {
            Subscription::none()
        } else {
            iced::time::every(self.game.gravity_interval()).map(|_| Message::Tick)
        };
        Subscription::batch([keyboard, gravity])
    }

    fn menu_view(&self) -> Element<'_, Message> {
        let mut actions = column![]
            .spacing(12)
            .width(Length::Fill)
            .align_x(iced::Alignment::Center);
        if !self.game.over {
            actions = actions.push(button("Continue").on_press(Message::Continue));
        }
        actions = actions
            .push(button("New game").on_press(Message::Restart))
            .push(text("Arrow keys: move and soft drop").color(rgb(0xa9bdc9)))
            .push(text("Up: rotate · Enter: hard drop · Esc: resume").color(rgb(0xa9bdc9)));

        container(
            column![
                text(if self.game.over {
                    "Game over"
                } else {
                    "Iced Blocks"
                })
                .size(42),
                text(format!(
                    "Score {}  ·  Lines {}  ·  Level {}",
                    self.game.score,
                    self.game.lines,
                    self.game.level()
                ))
                .color(rgb(0xa9bdc9)),
                container(actions).width(360).padding(24).style(panel_style),
            ]
            .spacing(18)
            .align_x(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(40)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(page_style)
        .into()
    }

    fn gameplay_view(&self) -> Element<'_, Message> {
        let board_state = self.board();
        let mut board_rows = Column::new().spacing(2);
        for y in 0..H {
            let mut cells = Row::new().spacing(2);
            for x in 0..W {
                let cell = board_state[y * W + x];
                cells = cells.push(container("").width(28).height(28).style(move |_| {
                    iced::widget::container::Style {
                        background: Some(Background::Color(
                            cell.map(|shape| rgb(COLORS[shape as usize]))
                                .unwrap_or_else(|| rgb(0x17242d)),
                        )),
                        border: Border {
                            color: if cell.is_some() {
                                rgb(0xd3fff8)
                            } else {
                                Color::TRANSPARENT
                            },
                            width: if cell.is_some() { 2.0 } else { 0.0 },
                            radius: 3.0.into(),
                        },
                        ..Default::default()
                    }
                }));
            }
            board_rows = board_rows.push(cells);
        }
        let board = container(board_rows).padding(8).style(board_style);
        let status = if self.game.over {
            "Stack topped out"
        } else if self.game.paused {
            "Ready — choose Start"
        } else {
            "In play"
        };
        let next_color = rgb(COLORS[self.game.next as usize]);
        let side_content = column![
            container(text(status))
                .padding(8)
                .style(move |_| status_style(self.game.over)),
            text("Next"),
            container(
                container("")
                    .width(30)
                    .height(30)
                    .style(move |_| colored_cell_style(next_color))
            )
            .width(90)
            .height(68)
            .padding(12)
            .style(dark_panel_style),
            text("Controls"),
            text("Arrow keys: move, drop, and rotate · Enter: hard drop · Esc: game menu.")
                .color(rgb(0xa9bdc9)),
        ]
        .width(210)
        .spacing(9)
        .padding(12);
        let side = container(side_content).style(panel_style);
        let header = row![
            text("Iced Blocks").size(22).width(Length::Fill),
            text(format!(
                "Score {}  ·  Lines {}  ·  Level {}",
                self.game.score,
                self.game.lines,
                self.game.level()
            )),
            text("Esc: game menu").color(rgb(0xa9bdc9)),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(14)
        .padding(10);
        container(
            column![
                container(header).width(Length::Fill).style(panel_style),
                row![board, side]
                    .spacing(18)
                    .align_y(iced::Alignment::Start),
            ]
            .spacing(16)
            .align_x(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(18)
        .style(page_style)
        .into()
    }

    fn view(&self) -> Element<'_, Message> {
        if self.menu_open || self.game.over {
            self.menu_view()
        } else {
            self.gameplay_view()
        }
    }
}

fn page_style(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        text_color: Some(rgb(0xeaf2f8)),
        background: Some(Background::Color(rgb(0x101820))),
        ..Default::default()
    }
}

fn panel_style(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        text_color: Some(rgb(0xeaf2f8)),
        background: Some(Background::Color(rgb(0x1d2a35))),
        border: Border {
            color: rgb(0x496274),
            width: 2.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    }
}

fn board_style(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(rgb(0x263845))),
        border: Border {
            color: rgb(0x496274),
            width: 2.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}

fn dark_panel_style(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(rgb(0x15232d))),
        ..Default::default()
    }
}

fn status_style(over: bool) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(rgb(if over {
            0x5a2134
        } else {
            0x15232d
        }))),
        border: Border {
            radius: 5.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn colored_cell_style(color: Color) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(color)),
        border: Border {
            color: rgb(0xf3eaff),
            width: 2.0,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn validate() -> Result<(), Box<dyn std::error::Error>> {
    let mut timing = Game::default();
    let level_one_gravity = timing.gravity_interval();
    timing.lines = 20;
    if timing.level() != 3 || timing.gravity_interval() >= level_one_gravity {
        return Err("gravity cadence does not accelerate with level".into());
    }
    let mut bag = Game::default();
    for _ in 0..2 {
        let mut seen = [false; 7];
        for _ in 0..7 {
            seen[bag.draw_piece() as usize] = true;
        }
        if !seen.into_iter().all(|piece| piece) {
            return Err("the seven-piece bag omitted or repeated a shape".into());
        }
    }
    let mut app = IcedBlocks::default();
    let mut ticks = 0;
    for _ in 0..400 {
        if app.game.over {
            break;
        }
        app.update(Message::Tick);
        ticks += 1;
    }
    if !app.game.over || ticks < 40 {
        return Err(
            format!("simulation stopped after {ticks} ticks without a normal top-out").into(),
        );
    }
    if app.board().len() != W * H {
        return Err("board view did not contain 200 cells".into());
    }
    println!("Iced view survived {ticks} gravity ticks and reached a normal top-out.");
    Ok(())
}

fn run() -> iced::Result {
    iced::application(IcedBlocks::default, IcedBlocks::update, IcedBlocks::view)
        .title("Iced Blocks")
        .subscription(IcedBlocks::subscription)
        .window_size((800.0, 720.0))
        .theme(|_: &IcedBlocks| Theme::Dark)
        .run()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match std::env::args().nth(1).as_deref() {
        None => run().map_err(Into::into),
        Some("--validate") => validate(),
        _ => Err("usage: iced-blocks [--validate]".into()),
    }
}
