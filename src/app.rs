use fltk::{
    app, button,
    enums::{Color, Event, FrameType},
    frame, group,
    prelude::*,
    window,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::thread;

use crate::config::Config;
use crate::constants;
use crate::domain::Region;
use crate::error::Error;
use crate::launcher;
use crate::ping;

const DEFAULT_BTN_COLOR: Color = Color::from_hex(0x2d2d44);
const HIGHLIGHT_BTN_COLOR: Color = Color::from_hex(0x4a3f6e);
const COUNTDOWN_SECONDS: i32 = 5;

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

    let button_refs: Vec<_> = create_region_buttons(default_region, sender, scale);
    add_buttons_to_layout(&mut layout, &button_refs, scale);

    let spacer = frame::Frame::default();
    layout.fixed(&spacer, 0);

    let mut countdown_label = create_countdown_label(scale);
    layout.fixed(&countdown_label, (14.0 * scale) as i32);

    layout.end();
    wind.end();

    // Countdown state
    let state = Rc::new(RefCell::new(CountdownState {
        seconds: COUNTDOWN_SECONDS,
        cancelled: false,
        sender,
    }));

    // Handle any window interaction to stop countdown
    let state_clone = state.clone();
    wind.handle(move |_wind, event| {
        if matches!(event, Event::Push | Event::KeyDown | Event::MouseWheel) {
            let mut s = state_clone.borrow_mut();
            if !s.cancelled && s.seconds > 0 {
                s.cancelled = true;
                sender.send(Message::CancelCountdown);
            }
        }
        false
    });

    wind.show();

    if config.quick_launch {
        countdown_label.set_label(&format!("Auto-launch in {}s...", state.borrow().seconds));
    }

    // Start countdown timer
    if config.quick_launch {
        schedule_countdown(state.clone());
    }

    // Start background ping measurements
    spawn_ping_threads(sender);

    // Handle events
    while app.wait() {
        if let Some(msg) = receiver.recv() {
            match msg {
                Message::Launch(region) => {
                    eprintln!("[d2rlauncher] Launching {}...", region);
                    state.borrow_mut().cancelled = true;
                    launch_region(&config, region)?;
                    return Ok(());
                }
                Message::AutoLaunch => {
                    if !state.borrow().cancelled {
                        eprintln!("[d2rlauncher] Auto-launching {}...", default_region);
                        launch_region(&config, default_region)?;
                        return Ok(());
                    }
                }
                Message::Countdown(secs) => {
                    if !state.borrow().cancelled {
                        countdown_label.set_label(&format!("Auto-launch in {}s...", secs));
                    }
                }
                Message::CancelCountdown => {
                    eprintln!("[d2rlauncher] Countdown cancelled");
                    countdown_label.set_label("");
                }
                Message::SetDefault(region) => {
                    eprintln!("[d2rlauncher] Setting default region to {}", region);
                    state.borrow_mut().cancelled = true;
                    countdown_label.set_label("");

                    for (btn, r) in &button_refs {
                        let mut btn = btn.borrow_mut();
                        if *r == region {
                            btn.set_color(HIGHLIGHT_BTN_COLOR);
                        } else {
                            btn.set_color(DEFAULT_BTN_COLOR);
                        }
                        btn.redraw();
                    }

                    config.default_region = Some(region);
                    config.save()?;
                    eprintln!("[d2rlauncher] Config saved");
                }
                Message::PingResult(region, ping_ms) => {
                    match ping_ms {
                        Some(ms) => eprintln!("[d2rlauncher] Ping {}: {}ms", region, ms),
                        None => eprintln!("[d2rlauncher] Ping {}: timeout", region),
                    }
                    update_button_ping(&button_refs, region, ping_ms);
                }
            }
        }
    }

    Ok(())
}

fn spawn_ping_threads(sender: app::Sender<Message>) {
    for region in Region::ALL {
        thread::spawn(move || loop {
            let ping_ms = ping::measure(region).map(|d| d.as_millis() as u32);
            sender.send(Message::PingResult(region, ping_ms));
            thread::sleep(std::time::Duration::from_secs(5));
        });
    }
}

fn update_button_ping(
    buttons: &[(Rc<RefCell<button::Button>>, Region)],
    region: Region,
    ping_ms: Option<u32>,
) {
    for (btn, r) in buttons {
        if *r == region {
            let mut btn = btn.borrow_mut();
            let (ping_str, color) = match ping_ms {
                Some(ms) => {
                    let color = if ms < 100 {
                        Color::from_hex(0x4ade80)
                    } else if ms < 200 {
                        Color::from_hex(0xfbbf24)
                    } else {
                        Color::from_hex(0xf87171)
                    };
                    (format!("{}ms", ms), color)
                }
                None => ("--ms".to_string(), Color::from_hex(0x888888)),
            };
            let label = format!("{} {} {}", region.flag(), region, ping_str);
            btn.set_label(&label);
            btn.set_label_color(color);
            btn.redraw();
            break;
        }
    }
}

struct CountdownState {
    seconds: i32,
    cancelled: bool,
    sender: app::Sender<Message>,
}

fn schedule_countdown(state: Rc<RefCell<CountdownState>>) {
    app::add_timeout3(1.0, move |_| {
        let mut s = state.borrow_mut();

        if s.cancelled {
            return;
        }

        s.seconds -= 1;

        if s.seconds > 0 {
            s.sender.send(Message::Countdown(s.seconds));
            drop(s);
            schedule_countdown(state.clone());
        } else {
            s.sender.send(Message::AutoLaunch);
        }
    });
}

fn launch_region(config: &Config, region: Region) -> Result<(), Error> {
    launcher::launch(config, region)?;
    Ok(())
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

fn create_region_buttons(
    default_region: Region,
    sender: app::Sender<Message>,
    scale: f32,
) -> Vec<(Rc<RefCell<button::Button>>, Region)> {
    Region::ALL
        .iter()
        .map(|&region| {
            let btn = create_region_button(region, default_region, sender, scale);
            (Rc::new(RefCell::new(btn)), region)
        })
        .collect()
}

fn create_region_button(
    region: Region,
    default_region: Region,
    sender: app::Sender<Message>,
    scale: f32,
) -> button::Button {
    let is_default = region == default_region;
    let label = format!("{} {}", region.flag(), region);
    let mut btn = button::Button::default().with_label(&label);

    btn.set_color(if is_default {
        HIGHLIGHT_BTN_COLOR
    } else {
        DEFAULT_BTN_COLOR
    });
    btn.set_label_color(Color::White);
    btn.set_label_size((11.0 * scale) as i32);
    btn.set_frame(FrameType::RoundedBox);

    btn.set_callback(move |_| {
        sender.send(Message::Launch(region));
    });

    btn.handle(move |_btn, event| {
        if event == Event::Push && app::event_button() == 3 {
            sender.send(Message::SetDefault(region));
            true
        } else {
            false
        }
    });

    btn
}

fn add_buttons_to_layout(
    layout: &mut group::Flex,
    buttons: &[(Rc<RefCell<button::Button>>, Region)],
    scale: f32,
) {
    let height = (28.0 * scale) as i32;
    for (btn, _) in buttons {
        layout.fixed(&*btn.borrow(), height);
    }
}

fn create_countdown_label(scale: f32) -> frame::Frame {
    let mut frame = frame::Frame::default();
    frame.set_label_size((10.0 * scale) as i32);
    frame.set_label_color(Color::from_hex(0xf0b90b));
    frame
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
