use std::time::Duration;
use twinkle_ui::prelude::*;
use twinkle_ui::{
    Application, ApplicationControllerEvent, ApplicationControllerInput, ControllerAxis,
    ControllerButton, ControllerInputOutcome, ControllerSnapshot, KeyEdge, Shortcut,
    ShortcutOutcome, View, ViewContext,
};

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
                // A tiny deterministic xorshift generator is sufficient here:
                // the bag supplies the distribution guarantee, while this only
                // chooses its permutation without adding an application dependency.
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
        for row in self.cells.chunks_exact(W) {
            if row.iter().all(Option::is_some) {
                cleared += 1;
            } else {
                kept.extend_from_slice(row);
            }
        }
        for _ in 0..cleared {
            kept.splice(0..0, std::iter::repeat_n(None, W));
        }
        self.cells = kept;
        self.lines += cleared as u32;
        self.score += [0, 100, 300, 500, 800][cleared.min(4)] * (self.level());
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

    fn soft_drop_interval(&self) -> Duration {
        Duration::from_millis((self.gravity_interval().as_millis() as u64 / 12).max(25))
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
            *self = Self {
                paused: false,
                ..Self::default()
            };
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

#[derive(Clone)]
enum Message {
    Continue,
    Restart,
}

struct NativeBlocks {
    game: Game,
    menu_open: bool,
    axis_x: i8,
    gravity_elapsed: Duration,
    horizontal_direction: i8,
    horizontal_elapsed: Duration,
    changed_view_ids: Option<Vec<UiId>>,
}

impl Default for NativeBlocks {
    fn default() -> Self {
        Self {
            game: Game::default(),
            menu_open: false,
            axis_x: 0,
            gravity_elapsed: Duration::ZERO,
            horizontal_direction: 0,
            horizontal_elapsed: Duration::ZERO,
            changed_view_ids: None,
        }
    }
}

impl NativeBlocks {
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

    fn reset_input(&mut self) {
        self.axis_x = 0;
        self.horizontal_direction = 0;
        self.horizontal_elapsed = Duration::ZERO;
    }

    fn mutate_game(&mut self, mutation: impl FnOnce(&mut Game)) {
        let before_board = self.board();
        let before_stats = (self.game.score, self.game.lines, self.game.level());
        let before_next = self.game.next;
        let before_status = (self.game.paused, self.game.over);
        mutation(&mut self.game);

        if before_status.1 != self.game.over {
            self.changed_view_ids = None;
            return;
        }

        let mut changed = before_board
            .into_iter()
            .zip(self.board())
            .enumerate()
            .filter_map(|(index, (before, after))| {
                (before != after).then(|| UiId::from(format!("cell-{index}")))
            })
            .collect::<Vec<_>>();
        if before_stats != (self.game.score, self.game.lines, self.game.level()) {
            changed.push(UiId::from("stats"));
        }
        if before_next != self.game.next {
            changed.push(UiId::from("next-cell"));
        }
        if before_status != (self.game.paused, self.game.over) {
            changed.push(UiId::from("status"));
        }
        if let Some(mut pending) = self.changed_view_ids.take() {
            pending.append(&mut changed);
            pending.sort();
            pending.dedup();
            self.changed_view_ids = Some(pending);
        } else {
            self.changed_view_ids = Some(changed);
        }
    }

    fn act(&mut self, action: &'static str) {
        self.mutate_game(|game| game.act(action));
    }

    fn menu_view(&self) -> AnyView<Message> {
        let mut actions = Column::new().gap(12.0);
        if !self.game.over {
            actions = actions.child(Button::new(Message::Continue, "Continue"));
        }
        actions = actions
            .child(Button::new(Message::Restart, "New game"))
            .child(Text::new("D-pad or left stick: move and soft drop").color(0xa9bdc9))
            .child(Text::new("A: rotate · B: hard drop · Start: resume").color(0xa9bdc9));

        AnyView::new(
            Column::new()
                .fill_width()
                .fill_height()
                .align_items(Align::Center)
                .justify_content(Justify::Center)
                .gap(18.0)
                .padding(Insets::all(40.0))
                .background(0x101820)
                .foreground(0xeaf2f8)
                .child(
                    Text::new(if self.game.over {
                        "Game over"
                    } else {
                        "Twinkle Blocks"
                    })
                    .font_size(42.0)
                    .line_height(50.0),
                )
                .child(
                    Text::new(format!(
                        "Score {}  ·  Lines {}  ·  Level {}",
                        self.game.score,
                        self.game.lines,
                        self.game.level()
                    ))
                    .color(0xa9bdc9),
                )
                .child(
                    Container::new()
                        .width(360.0)
                        .padding(Insets::all(24.0))
                        .background(0x1d2a35)
                        .border(0x496274, 2.0)
                        .radius(12.0)
                        .child(actions),
                ),
        )
    }

    fn gameplay_view(&self) -> AnyView<Message> {
        let cells = self.board().into_iter().enumerate().map(|(index, cell)| {
            let color = cell.map(|shape| COLORS[shape as usize]).unwrap_or(0x17242d);
            let cell_view = Container::new()
                .id(UiId::from(format!("cell-{index}")))
                .width(28.0)
                .height(28.0)
                .background(color)
                .radius(3.0);
            if cell.is_some() {
                cell_view.border(0xd3fff8, 2.0)
            } else {
                cell_view
            }
        });
        let board = Grid::fixed(W)
            .gap(2.0)
            .padding(Insets::all(8.0))
            .background(0x263845)
            .border_value(Border::new(0x496274, 2.0))
            .radius(8.0)
            .children(cells);
        let status = if self.game.over {
            "Stack topped out"
        } else if self.game.paused {
            "Ready — choose Start"
        } else {
            "In play"
        };
        let side = Column::new()
            .width(210.0)
            .gap(9.0)
            .padding(Insets::all(12.0))
            .background(0x1d2a35)
            .child(
                Container::new()
                    .id(id!(status))
                    .padding(Insets::all(8.0))
                    .background(if self.game.over { 0x5a2134 } else { 0x15232d })
                    .radius(5.0)
                    .child(Text::new(status)),
            )
            .child(Text::new("Next"))
            .child(
                Container::new()
                    .width(90.0)
                    .height(68.0)
                    .padding(Insets::all(12.0))
                    .background(0x15232d)
                    .child(
                        Container::new()
                            .id(id!(next_cell))
                            .width(30.0)
                            .height(30.0)
                            .background(COLORS[self.game.next as usize])
                            .border(0xf3eaff, 2.0),
                    ),
            )
            .child(Text::new("Controls"))
            .child(
                Text::new("D-pad / left stick: move and drop · A: rotate · B: hard drop · Start: game menu. Keyboard: ↑/↓, Alt+←/→, Enter, Escape.")
                    .color(0xa9bdc9)
                    .wrap(true),
            );

        AnyView::new(
            Column::new()
                .fill_width()
                .fill_height()
                .padding(Insets::all(18.0))
                .gap(16.0)
                .background(0x101820)
                .foreground(0xeaf2f8)
                .child(
                    Row::new()
                        .align_items(Align::Center)
                        .gap(14.0)
                        .padding(Insets::all(10.0))
                        .background(0x1d2a35)
                        .child(Text::new("Twinkle Blocks").font_size(22.0).grow(1.0))
                        .child(
                            Text::new(format!(
                                "Score {}  ·  Lines {}  ·  Level {}",
                                self.game.score,
                                self.game.lines,
                                self.game.level()
                            ))
                            .id(id!(stats)),
                        )
                        .child(Text::new("Start / Esc: game menu").color(0xa9bdc9)),
                )
                .child(
                    Row::new()
                        .gap(18.0)
                        .justify_content(Justify::Center)
                        .align_items(Align::Start)
                        .child(board)
                        .child(side),
                ),
        )
    }
}

impl Application for NativeBlocks {
    type Message = Message;

    fn update(&mut self, message: Message) {
        match message {
            Message::Continue => {
                self.menu_open = false;
                self.reset_input();
                self.changed_view_ids = None;
            }
            Message::Restart => {
                self.game.act("restart");
                self.menu_open = false;
                self.gravity_elapsed = Duration::ZERO;
                self.reset_input();
                self.changed_view_ids = None;
            }
        }
    }

    fn view(&self, _context: ViewContext) -> impl View<Message> {
        if self.menu_open || self.game.over {
            self.menu_view()
        } else {
            self.gameplay_view()
        }
    }

    fn controller_input(&mut self, event: ApplicationControllerEvent) -> ControllerInputOutcome {
        use ApplicationControllerInput as Input;
        if matches!(&event.input, Input::RouteNeutralized | Input::Disconnected) {
            self.axis_x = 0;
        }
        if let Input::Button {
            button: ControllerButton::Start,
            edge: KeyEdge::Pressed,
            repeat,
        } = event.input
        {
            if repeat || self.game.over {
                return ControllerInputOutcome::handled(false);
            }
            self.menu_open = !self.menu_open;
            self.reset_input();
            self.changed_view_ids = None;
            return ControllerInputOutcome::handled(true);
        }
        if let Input::Button {
            button: ControllerButton::DPadDown,
            edge,
            repeat,
        } = event.input
        {
            let active = !self.menu_open && !self.game.paused && !self.game.over;
            let changed = active && edge == KeyEdge::Pressed && !repeat;
            if changed {
                self.act("down");
            }
            return if active {
                ControllerInputOutcome::handled(changed)
            } else {
                ControllerInputOutcome::unhandled()
            };
        }
        if self.menu_open || self.game.paused || self.game.over {
            return ControllerInputOutcome::unhandled();
        }
        let action = match event.input {
            Input::Button {
                button,
                edge: KeyEdge::Pressed,
                repeat,
            } => {
                let action = match button {
                    ControllerButton::DPadLeft => Some("left"),
                    ControllerButton::DPadRight => Some("right"),
                    ControllerButton::DPadDown | ControllerButton::West => Some("down"),
                    ControllerButton::DPadUp | ControllerButton::South => Some("rotate"),
                    ControllerButton::East => Some("drop"),
                    _ => None,
                };
                if repeat && !matches!(action, Some("left" | "right" | "down")) {
                    return ControllerInputOutcome::handled(false);
                }
                action
            }
            Input::Axis {
                axis: ControllerAxis::LeftX,
                value,
            } => {
                let direction = if value < -0.65 {
                    -1
                } else if value > 0.65 {
                    1
                } else {
                    0
                };
                let action = (direction != 0 && direction != self.axis_x)
                    .then_some(if direction < 0 { "left" } else { "right" });
                self.axis_x = direction;
                action
            }
            Input::Axis {
                axis: ControllerAxis::LeftY,
                ..
            }
            | Input::RouteNeutralized
            | Input::Disconnected => return ControllerInputOutcome::handled(false),
            _ => return ControllerInputOutcome::unhandled(),
        };
        if let Some(action) = action {
            self.act(action);
        }
        ControllerInputOutcome::handled(action.is_some())
    }

    fn shortcut_outcome(&mut self, shortcut: Shortcut) -> ShortcutOutcome {
        if shortcut == Shortcut::Escape {
            if self.game.over {
                return ShortcutOutcome::from_changed(false);
            }
            self.menu_open = !self.menu_open;
            self.reset_input();
            self.changed_view_ids = None;
            return ShortcutOutcome::from_changed(true);
        }
        if self.menu_open {
            return ShortcutOutcome::from_changed(false);
        }
        let action = match shortcut {
            Shortcut::NavigateUp => Some("rotate"),
            Shortcut::NavigateDown => Some("down"),
            Shortcut::Back => Some("left"),
            Shortcut::Forward => Some("right"),
            Shortcut::Submit => Some("drop"),
            Shortcut::Reload => Some("restart"),
            _ => None,
        };
        if let Some(action) = action {
            if (self.game.paused || self.game.over) && action != "restart" {
                return ShortcutOutcome::from_changed(false);
            }
            self.act(action);
            ShortcutOutcome::from_changed(true)
        } else {
            ShortcutOutcome::from_changed(false)
        }
    }

    fn poll_interval(&self) -> Option<Duration> {
        (!self.game.paused && !self.game.over && !self.menu_open)
            .then(|| self.game.gravity_interval())
    }

    fn poll_interval_controller(&self, input: &ControllerSnapshot) -> Option<Duration> {
        (!self.game.paused && !self.game.over && !self.menu_open).then(|| {
            let mut interval = self.game.gravity_interval();
            if input.button_down(&ControllerButton::DPadDown)
                || input.axis(&ControllerAxis::LeftY) > 0.65
            {
                interval = interval.min(self.game.soft_drop_interval());
            }
            if input.button_down(&ControllerButton::DPadLeft)
                || input.button_down(&ControllerButton::DPadRight)
                || input.axis(&ControllerAxis::LeftX).abs() > 0.65
            {
                interval = interval.min(Duration::from_millis(50));
            }
            interval
        })
    }

    fn poll_controller(&mut self, input: &ControllerSnapshot) -> bool {
        if self.game.paused || self.game.over || self.menu_open {
            return false;
        }
        let interval = self
            .poll_interval_controller(input)
            .unwrap_or_else(|| self.game.gravity_interval());
        let soft_drop = input.button_down(&ControllerButton::DPadDown)
            || input.axis(&ControllerAxis::LeftY) > 0.65;
        if soft_drop {
            self.act("down");
        } else {
            self.gravity_elapsed += interval;
            if self.gravity_elapsed >= self.game.gravity_interval() {
                self.gravity_elapsed = self
                    .gravity_elapsed
                    .saturating_sub(self.game.gravity_interval());
                self.mutate_game(Game::gravity_tick);
            }
        }
        let axis_x = input.axis(&ControllerAxis::LeftX);
        let horizontal = if input.button_down(&ControllerButton::DPadLeft) || axis_x < -0.65 {
            -1
        } else if input.button_down(&ControllerButton::DPadRight) || axis_x > 0.65 {
            1
        } else {
            0
        };
        if horizontal == 0 || horizontal != self.horizontal_direction {
            self.horizontal_direction = horizontal;
            self.horizontal_elapsed = Duration::ZERO;
        } else {
            self.horizontal_elapsed += interval;
            if self.horizontal_elapsed >= Duration::from_millis(150) {
                self.act(if horizontal < 0 { "left" } else { "right" });
                self.horizontal_elapsed = Duration::from_millis(100);
            }
        }
        true
    }

    fn title(&self) -> &str {
        "Twinkle Blocks"
    }
    fn initial_size(&self) -> (u32, u32) {
        (800, 720)
    }

    fn take_changed_view_ids(&mut self) -> Option<Vec<UiId>> {
        self.changed_view_ids.take()
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
    let mut app = NativeBlocks::default();
    let mut ticks = 0;
    for _ in 0..400 {
        if !app.poll_controller(&ControllerSnapshot::default()) {
            break;
        }
        ticks += 1;
    }
    if !app.game.over || ticks < 40 {
        return Err(format!("polling stopped after {ticks} ticks without a normal top-out").into());
    }
    let mut host = twinkle_ui::UiHost::new(NativeBlocks::default(), 800, 720);
    let tree = host.inspect().resources;
    println!(
        "Gameplay component tree: {} bytes ({:.1} KiB), {} resolved nodes, {} retained nodes, {} paint primitives, {} accessibility nodes.",
        tree.estimated_retained_bytes,
        tree.estimated_retained_bytes as f64 / 1024.0,
        tree.node_count,
        tree.retained_node_count,
        tree.paint_primitive_count,
        tree.accessibility_node_count,
    );
    host.application_mut().act("left");
    let incremental = host.step(twinkle_ui::HostBatch {
        application_changed: true,
        ..twinkle_ui::HostBatch::default()
    });
    if incremental.telemetry.nodes_measured >= tree.node_count {
        return Err(format!(
            "targeted native update measured {} of {} nodes",
            incremental.telemetry.nodes_measured, tree.node_count
        )
        .into());
    }
    println!(
        "Targeted movement measured {} of {} nodes and emitted {} commands.",
        incremental.telemetry.nodes_measured,
        tree.node_count,
        incremental.telemetry.paint_commands_emitted,
    );
    println!("Native Rust view survived {ticks} gravity ticks and reached a normal top-out.");
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match std::env::args().nth(1).as_deref() {
        None => twinkle_ui::run(NativeBlocks::default()),
        Some("--validate") => validate(),
        _ => Err("usage: twinkle-blocks [--validate]".into()),
    }
}
