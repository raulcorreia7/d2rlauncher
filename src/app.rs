use fltk::{
    app, button,
    enums::{Color, Event, FrameType},
    frame, group, image,
    prelude::*,
    window,
};
use std::thread;
use std::{cell::RefCell, rc::Rc, time::Duration};

use crate::config::Config;
use crate::constants;
use crate::domain::Region;
use crate::error::Error;
use crate::launcher;
use crate::ping;

const ICON_DATA: &[u8] = include_bytes!("../icon.png");

const DEFAULT_BTN_COLOR: Color = Color::from_hex(0x2d2d44);
const HIGHLIGHT_BTN_COLOR: Color = Color::from_hex(0x4a3f6e);
const COUNTDOWN_SECONDS: i32 = 5;
const PING_REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const COUNTDOWN_LABEL_HEIGHT: i32 = 14;
const REGION_BUTTON_HEIGHT: i32 = 28;

pub fn run() -> Result<(), Error> {
    eprintln!("[d2rlauncher] Starting...");
    let app = app::App::default().with_scheme(app::Scheme::Gleam);
    setup_theme();

    eprintln!("[d2rlauncher] Loading config...");
    let mut config = Config::load()?;
    let default_region = config.default_region.unwrap_or_default();
    eprintln!("[d2rlauncher] Default region: {}", default_region);
    eprintln!("[d2rlauncher] Quick launch: {}", config.quick_launch);

    let scale = app::screen_scale(0);
    let (win_width, win_height) = scaled_window_size(scale);

    let mut wind = create_window(win_width, win_height);
    let mut layout = create_layout(scale);

    let (sender, receiver) = app::channel::<Message>();

    let mut ui = Ui::new(default_region, sender, scale, &mut layout);

    layout.end();
    wind.end();

    let state = Rc::new(RefCell::new(CountdownState::new(COUNTDOWN_SECONDS)));

    bind_countdown_cancel(&mut wind, state.clone(), sender);

    wind.show();

    if config.quick_launch {
        ui.show_countdown(state.borrow().remaining_seconds());
        schedule_countdown(state.clone(), sender);
    }

    spawn_ping_threads(sender);

    while app.wait() {
        if let Some(msg) = receiver.recv() {
            if let Some(region) =
                handle_message(msg, &mut config, default_region, state.as_ref(), &mut ui)?
            {
                launcher::launch(&config, region)?;
                return Ok(());
            }
        }
    }

    Ok(())
}

fn spawn_ping_threads(sender: app::Sender<Message>) {
    for region in Region::ALL {
        thread::spawn(move || {
            let monitor = ping::PingMonitor::new();
            loop {
                let ping_ms = monitor
                    .as_ref()
                    .and_then(|monitor| monitor.measure(region))
                    .map(|duration| duration.as_millis() as u32);

                sender.send(Message::PingResult(region, ping_ms));
                thread::sleep(PING_REFRESH_INTERVAL);
            }
        });
    }
}

fn bind_countdown_cancel(
    wind: &mut window::Window,
    state: Rc<RefCell<CountdownState>>,
    sender: app::Sender<Message>,
) {
    wind.handle(move |_wind, event| {
        if matches!(event, Event::Push | Event::KeyDown | Event::MouseWheel)
            && state.borrow_mut().cancel()
        {
            sender.send(Message::CancelCountdown);
        }
        false
    });
}

fn handle_message(
    msg: Message,
    config: &mut Config,
    auto_launch_region: Region,
    state: &RefCell<CountdownState>,
    ui: &mut Ui,
) -> Result<Option<Region>, Error> {
    match msg {
        Message::Launch(region) => {
            eprintln!("[d2rlauncher] Launching {}...", region);
            state.borrow_mut().cancel();
            Ok(Some(region))
        }
        Message::AutoLaunch if state.borrow().is_cancelled() => Ok(None),
        Message::AutoLaunch => {
            eprintln!("[d2rlauncher] Auto-launching {}...", auto_launch_region);
            Ok(Some(auto_launch_region))
        }
        Message::Countdown(_) if state.borrow().is_cancelled() => Ok(None),
        Message::Countdown(secs) => {
            ui.show_countdown(secs);
            Ok(None)
        }
        Message::CancelCountdown => {
            eprintln!("[d2rlauncher] Countdown cancelled");
            ui.clear_countdown();
            Ok(None)
        }
        Message::SetDefault(region) => {
            eprintln!("[d2rlauncher] Setting default region to {}", region);
            state.borrow_mut().cancel();
            ui.clear_countdown();
            ui.set_default_region(region);

            config.default_region = Some(region);
            config.save()?;
            eprintln!("[d2rlauncher] Config saved");
            Ok(None)
        }
        Message::PingResult(region, ping_ms) => {
            log_ping_result(region, ping_ms);
            ui.update_ping(region, ping_ms);
            Ok(None)
        }
    }
}

fn schedule_countdown(state: Rc<RefCell<CountdownState>>, sender: app::Sender<Message>) {
    app::add_timeout3(1.0, move |_| match state.borrow_mut().tick() {
        CountdownProgress::Cancelled => {}
        CountdownProgress::Running(secs) => {
            sender.send(Message::Countdown(secs));
            schedule_countdown(state.clone(), sender);
        }
        CountdownProgress::Complete => {
            sender.send(Message::AutoLaunch);
        }
    });
}

fn setup_theme() {
    app::background(0x1a, 0x1a, 0x2e);
    app::background2(0x2d, 0x2d, 0x44);
    app::foreground(0xff, 0xff, 0xff);
}

fn scaled_window_size(scale: f32) -> (i32, i32) {
    (
        (constants::WINDOW_WIDTH as f32 * scale) as i32,
        (constants::WINDOW_HEIGHT as f32 * scale) as i32,
    )
}

fn create_window(width: i32, height: i32) -> window::Window {
    let mut wind = window::Window::default()
        .with_size(width, height)
        .with_label(constants::APP_TITLE);
    wind.set_color(Color::from_hex(0x1a1a2e));
    wind.make_resizable(false);

    if let Ok(icon) = image::PngImage::from_data(ICON_DATA) {
        wind.set_icon(Some(icon));
    }

    wind
}

fn create_layout(scale: f32) -> group::Flex {
    let margin = (6.0 * scale) as i32;
    let spacing = (3.0 * scale) as i32;
    let mut col = group::Flex::default_fill().column();
    col.set_margins(margin, margin, margin, margin);
    col.set_spacing(spacing);
    col
}

fn log_ping_result(region: Region, ping_ms: Option<u32>) {
    match ping_ms {
        Some(ms) => eprintln!("[d2rlauncher] Ping {}: {}ms", region, ms),
        None => eprintln!("[d2rlauncher] Ping {}: timeout", region),
    }
}

fn ping_display(ping_ms: Option<u32>) -> (String, Color) {
    match ping_ms {
        Some(ms) => {
            let color = if ms < 100 {
                Color::from_hex(0x4ade80)
            } else if ms < 200 {
                Color::from_hex(0xfbbf24)
            } else {
                Color::from_hex(0xf87171)
            };
            (format!("{ms}ms"), color)
        }
        None => ("--ms".to_string(), Color::from_hex(0x888888)),
    }
}

fn style_region_button(btn: &mut button::Button, is_default: bool, scale: f32) {
    btn.set_color(if is_default {
        HIGHLIGHT_BTN_COLOR
    } else {
        DEFAULT_BTN_COLOR
    });
    btn.set_label_color(Color::White);
    btn.set_label_size((11.0 * scale) as i32);
    btn.set_frame(FrameType::RoundedBox);
}

fn create_countdown_label(scale: f32) -> frame::Frame {
    let mut frame = frame::Frame::default();
    frame.set_label_size((10.0 * scale) as i32);
    frame.set_label_color(Color::from_hex(0xf0b90b));
    frame
}

struct Ui {
    buttons: Vec<RegionButton>,
    countdown_label: frame::Frame,
}

impl Ui {
    fn new(
        default_region: Region,
        sender: app::Sender<Message>,
        scale: f32,
        layout: &mut group::Flex,
    ) -> Self {
        let buttons = Region::ALL
            .iter()
            .map(|&region| RegionButton::new(region, default_region, sender, scale))
            .collect::<Vec<_>>();

        let button_height = (REGION_BUTTON_HEIGHT as f32 * scale) as i32;
        for button in &buttons {
            layout.fixed(&button.widget, button_height);
        }

        let spacer = frame::Frame::default();
        layout.fixed(&spacer, 0);

        let countdown_label_height = (COUNTDOWN_LABEL_HEIGHT as f32 * scale) as i32;
        let countdown_label = create_countdown_label(scale);
        layout.fixed(&countdown_label, countdown_label_height);

        Self {
            buttons,
            countdown_label,
        }
    }

    fn show_countdown(&mut self, seconds: i32) {
        self.countdown_label
            .set_label(&format!("Auto-launch in {seconds}s..."));
    }

    fn clear_countdown(&mut self) {
        self.countdown_label.set_label("");
    }

    fn set_default_region(&mut self, region: Region) {
        for button in &mut self.buttons {
            button.set_default(button.region == region);
        }
    }

    fn update_ping(&mut self, region: Region, ping_ms: Option<u32>) {
        for button in &mut self.buttons {
            if button.region == region {
                button.update_ping(ping_ms);
                break;
            }
        }
    }
}

struct RegionButton {
    widget: button::Button,
    region: Region,
}

impl RegionButton {
    fn new(
        region: Region,
        default_region: Region,
        sender: app::Sender<Message>,
        scale: f32,
    ) -> Self {
        let label = format!("{} {}", region.flag(), region);
        let mut widget = button::Button::default().with_label(&label);
        style_region_button(&mut widget, region == default_region, scale);

        widget.set_callback(move |_| {
            sender.send(Message::Launch(region));
        });

        widget.handle(move |_btn, event| {
            if event == Event::Push && app::event_button() == 3 {
                sender.send(Message::SetDefault(region));
                true
            } else {
                false
            }
        });

        Self { widget, region }
    }

    fn set_default(&mut self, is_default: bool) {
        self.widget.set_color(if is_default {
            HIGHLIGHT_BTN_COLOR
        } else {
            DEFAULT_BTN_COLOR
        });
        self.widget.redraw();
    }

    fn update_ping(&mut self, ping_ms: Option<u32>) {
        let (ping_text, color) = ping_display(ping_ms);
        let label = format!("{} {} {}", self.region.flag(), self.region, ping_text);
        self.widget.set_label(&label);
        self.widget.set_label_color(color);
        self.widget.redraw();
    }
}

struct CountdownState {
    remaining_seconds: i32,
    cancelled: bool,
}

impl CountdownState {
    fn new(remaining_seconds: i32) -> Self {
        Self {
            remaining_seconds,
            cancelled: false,
        }
    }

    fn remaining_seconds(&self) -> i32 {
        self.remaining_seconds
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn cancel(&mut self) -> bool {
        if self.cancelled || self.remaining_seconds <= 0 {
            return false;
        }

        self.cancelled = true;
        true
    }

    fn tick(&mut self) -> CountdownProgress {
        if self.cancelled {
            return CountdownProgress::Cancelled;
        }

        self.remaining_seconds -= 1;
        if self.remaining_seconds > 0 {
            CountdownProgress::Running(self.remaining_seconds)
        } else {
            CountdownProgress::Complete
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CountdownProgress {
    Cancelled,
    Running(i32),
    Complete,
}

#[derive(Debug, Clone)]
enum Message {
    Launch(Region),
    AutoLaunch,
    Countdown(i32),
    CancelCountdown,
    SetDefault(Region),
    PingResult(Region, Option<u32>),
}
