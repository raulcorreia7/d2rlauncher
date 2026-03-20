use fltk::{
    app, button,
    enums::{Align, Color, FrameType},
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
use crate::logln;
use crate::ping;

const ICON_DATA: &[u8] = include_bytes!("../icon.png");

const WINDOW_COLOR: Color = Color::from_hex(0x0f172a);
const SURFACE_COLOR: Color = Color::from_hex(0x182338);
const IDLE_REGION_COLOR: Color = Color::from_hex(0x22304a);
const SELECTED_REGION_COLOR: Color = Color::from_hex(0x2563eb);
const DEFAULT_REGION_COLOR: Color = Color::from_hex(0x0f766e);
const SELECTED_DEFAULT_REGION_COLOR: Color = Color::from_hex(0xd97706);
const PRIMARY_ACTION_COLOR: Color = Color::from_hex(0xf59e0b);
const SECONDARY_ACTION_COLOR: Color = Color::from_hex(0x0f766e);
const CANCEL_ACTION_COLOR: Color = Color::from_hex(0x475569);
const MUTED_TEXT_COLOR: Color = Color::from_hex(0x94a3b8);
const COUNTDOWN_TEXT_COLOR: Color = Color::from_hex(0xfbbf24);

const COUNTDOWN_SECONDS: i32 = 5;
const PING_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

const TITLE_HEIGHT: i32 = 28;
const SUBTITLE_HEIGHT: i32 = 18;
const REGION_BUTTON_HEIGHT: i32 = 56;
const STATUS_HEIGHT: i32 = 34;
const COUNTDOWN_ROW_HEIGHT: i32 = 30;
const ACTION_ROW_HEIGHT: i32 = 38;
const CANCEL_BUTTON_WIDTH: i32 = 116;

pub fn run() -> Result<(), Error> {
    logln!("[d2rlauncher] Starting...");
    app::keyboard_screen_scaling(false);

    let app = app::App::default().with_scheme(app::Scheme::Gleam);
    setup_theme();

    logln!("[d2rlauncher] Loading config...");
    let mut config = Config::load()?;
    let default_region = config.default_region.unwrap_or_default();
    logln!("[d2rlauncher] Default region: {}", default_region);
    logln!("[d2rlauncher] Quick launch: {}", config.quick_launch);

    let scale = UiScale::detect();
    let mut wind = create_window(scale);
    let mut layout = create_layout(scale);

    let (sender, receiver) = app::channel::<Message>();

    let mut selection = SelectionState::new(default_region);
    let mut ui = Ui::new(
        selection.selected_region,
        selection.default_region,
        sender,
        scale,
        &mut layout,
    );

    layout.end();
    wind.end();

    let countdown = Rc::new(RefCell::new(CountdownState::new(COUNTDOWN_SECONDS)));

    wind.show();

    if config.quick_launch {
        ui.show_countdown(countdown.borrow().remaining_seconds());
        schedule_countdown(countdown.clone(), sender);
    }

    spawn_ping_threads(sender);

    while app.wait() {
        if let Some(msg) = receiver.recv() {
            if let Some(region) = handle_message(
                msg,
                &mut config,
                &mut selection,
                countdown.as_ref(),
                &mut ui,
            )? {
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

fn handle_message(
    msg: Message,
    config: &mut Config,
    selection: &mut SelectionState,
    countdown: &RefCell<CountdownState>,
    ui: &mut Ui,
) -> Result<Option<Region>, Error> {
    match msg {
        Message::SelectRegion(region) => {
            cancel_countdown(countdown, ui);
            selection.selected_region = region;
            ui.set_selected_region(region);
            Ok(None)
        }
        Message::LaunchSelected => {
            let region = selection.selected_region;
            cancel_countdown(countdown, ui);
            logln!("[d2rlauncher] Launching {}...", region);
            Ok(Some(region))
        }
        Message::AutoLaunch if countdown.borrow().is_cancelled() => Ok(None),
        Message::AutoLaunch => {
            let region = selection.selected_region;
            logln!("[d2rlauncher] Auto-launching {}...", region);
            Ok(Some(region))
        }
        Message::Countdown(_) if countdown.borrow().is_cancelled() => Ok(None),
        Message::Countdown(secs) => {
            ui.show_countdown(secs);
            Ok(None)
        }
        Message::CancelCountdown => {
            cancel_countdown(countdown, ui);
            Ok(None)
        }
        Message::SetDefaultSelected => {
            cancel_countdown(countdown, ui);
            let region = selection.selected_region;
            logln!("[d2rlauncher] Setting default region to {}", region);

            selection.default_region = region;
            ui.set_default_region(region);

            config.default_region = Some(region);
            config.save()?;
            logln!("[d2rlauncher] Config saved");
            Ok(None)
        }
        Message::PingResult(region, ping_ms) => {
            log_ping_result(region, ping_ms);
            ui.update_ping(region, ping_ms);
            Ok(None)
        }
    }
}

fn cancel_countdown(countdown: &RefCell<CountdownState>, ui: &mut Ui) {
    if countdown.borrow_mut().cancel() {
        logln!("[d2rlauncher] Countdown cancelled");
    }

    ui.clear_countdown();
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
    app::background(0x0f, 0x17, 0x2a);
    app::background2(0x18, 0x23, 0x38);
    app::foreground(0xff, 0xff, 0xff);
}

fn create_window(scale: UiScale) -> window::Window {
    let mut wind = window::Window::default()
        .with_size(
            scale.px(constants::WINDOW_WIDTH),
            scale.px(constants::WINDOW_HEIGHT),
        )
        .with_label(constants::APP_TITLE);
    wind.set_color(WINDOW_COLOR);
    wind.make_resizable(false);

    if let Ok(icon) = image::PngImage::from_data(ICON_DATA) {
        wind.set_icon(Some(icon));
    }

    wind
}

fn create_layout(scale: UiScale) -> group::Flex {
    let mut col = group::Flex::default_fill().column();
    col.set_margins(scale.px(12), scale.px(12), scale.px(12), scale.px(12));
    col.set_spacing(scale.px(8));
    col
}

fn log_ping_result(region: Region, ping_ms: Option<u32>) {
    match ping_ms {
        Some(ms) => logln!("[d2rlauncher] Ping {}: {}ms", region, ms),
        None => logln!("[d2rlauncher] Ping {}: timeout", region),
    }
}

#[derive(Debug, Clone, Copy)]
struct UiScale {
    factor: f32,
}

impl UiScale {
    fn detect() -> Self {
        Self {
            factor: app::screen_scale(0).max(1.0),
        }
    }

    fn px(self, base: i32) -> i32 {
        ((base as f32) * self.factor).round().max(1.0) as i32
    }
}

#[derive(Debug, Clone, Copy)]
struct PingPresentation {
    color: Color,
    description: &'static str,
}

fn ping_presentation(ping_ms: Option<u32>) -> PingPresentation {
    match ping_ms {
        Some(ms) if ms < 80 => PingPresentation {
            color: Color::from_hex(0x4ade80),
            description: "Excellent",
        },
        Some(ms) if ms < 140 => PingPresentation {
            color: Color::from_hex(0xfacc15),
            description: "Good",
        },
        Some(_) => PingPresentation {
            color: Color::from_hex(0xf87171),
            description: "High ping",
        },
        None => PingPresentation {
            color: MUTED_TEXT_COLOR,
            description: "Measuring ping",
        },
    }
}

fn region_button_color(selected: bool, is_default: bool) -> Color {
    match (selected, is_default) {
        (true, true) => SELECTED_DEFAULT_REGION_COLOR,
        (true, false) => SELECTED_REGION_COLOR,
        (false, true) => DEFAULT_REGION_COLOR,
        (false, false) => IDLE_REGION_COLOR,
    }
}

fn region_button_label(
    region: Region,
    ping_ms: Option<u32>,
    selected: bool,
    is_default: bool,
) -> String {
    let badge = match (selected, is_default) {
        (true, true) => "Selected • Default region",
        (true, false) => "Selected",
        (false, true) => "Default region",
        (false, false) => "",
    };

    let ping = ping_presentation(ping_ms);
    let ping_text = match ping_ms {
        Some(ms) => format!("{} • {ms} ms", ping.description),
        None => ping.description.to_string(),
    };

    let details = if badge.is_empty() {
        ping_text
    } else {
        format!("{badge} • {ping_text}")
    };

    format!("{} {}\n{}", region.flag(), region, details)
}

fn selection_summary(
    region: Region,
    default_region: Region,
    ping_ms: Option<u32>,
) -> (String, Color) {
    let ping = ping_presentation(ping_ms);
    let summary = match ping_ms {
        Some(ms) if region == default_region => {
            format!(
                "Selected: {region} • {} • {ms} ms • Default region",
                ping.description
            )
        }
        Some(ms) => format!("Selected: {region} • {} • {ms} ms", ping.description),
        None if region == default_region => {
            format!("Selected: {region} • {} • Default region", ping.description)
        }
        None => format!("Selected: {region} • {}", ping.description),
    };

    (summary, ping.color)
}

fn countdown_message(region: Region, seconds: i32) -> String {
    format!("Auto-launching {region} in {seconds}s")
}

fn style_title(frame: &mut frame::Frame, scale: UiScale) {
    frame.set_label_size(scale.px(20));
    frame.set_label_color(Color::White);
    frame.set_align(Align::Left | Align::Inside);
}

fn style_subtitle(frame: &mut frame::Frame, scale: UiScale) {
    frame.set_label_size(scale.px(11));
    frame.set_label_color(MUTED_TEXT_COLOR);
    frame.set_align(Align::Left | Align::Inside);
}

fn style_status_frame(frame: &mut frame::Frame, scale: UiScale, color: Color) {
    frame.set_frame(FrameType::RoundedBox);
    frame.set_color(SURFACE_COLOR);
    frame.set_label_color(color);
    frame.set_label_size(scale.px(11));
    frame.set_align(Align::Left | Align::Inside);
}

fn style_region_button(btn: &mut button::Button, scale: UiScale) {
    btn.set_frame(FrameType::RoundedBox);
    btn.set_label_size(scale.px(13));
    btn.set_label_color(Color::White);
    btn.set_align(Align::Left | Align::Inside);
}

fn style_action_button(btn: &mut button::Button, scale: UiScale, color: Color) {
    btn.set_frame(FrameType::RoundedBox);
    btn.set_color(color);
    btn.set_label_color(Color::White);
    btn.set_label_size(scale.px(12));
}

struct Ui {
    buttons: Vec<RegionButton>,
    summary_label: frame::Frame,
    countdown_label: frame::Frame,
    launch_button: button::Button,
    default_button: button::Button,
    cancel_button: button::Button,
    selected_region: Region,
    default_region: Region,
    countdown_seconds: Option<i32>,
}

impl Ui {
    fn new(
        selected_region: Region,
        default_region: Region,
        sender: app::Sender<Message>,
        scale: UiScale,
        layout: &mut group::Flex,
    ) -> Self {
        let mut title = frame::Frame::default().with_label("Diablo II: Resurrected");
        style_title(&mut title, scale);
        layout.fixed(&title, scale.px(TITLE_HEIGHT));

        let mut subtitle = frame::Frame::default().with_label("Choose your region, then launch.");
        style_subtitle(&mut subtitle, scale);
        layout.fixed(&subtitle, scale.px(SUBTITLE_HEIGHT));

        let buttons = Region::ALL
            .iter()
            .map(|&region| RegionButton::new(region, sender, scale))
            .collect::<Vec<_>>();

        let button_height = scale.px(REGION_BUTTON_HEIGHT);
        for button in &buttons {
            layout.fixed(&button.widget, button_height);
        }

        let mut summary_label = frame::Frame::default();
        style_status_frame(&mut summary_label, scale, Color::White);
        layout.fixed(&summary_label, scale.px(STATUS_HEIGHT));

        let mut countdown_row = group::Flex::default().row();
        countdown_row.set_spacing(scale.px(8));

        let mut countdown_label = frame::Frame::default();
        style_status_frame(&mut countdown_label, scale, COUNTDOWN_TEXT_COLOR);

        let mut cancel_button = button::Button::default().with_label("Cancel");
        style_action_button(&mut cancel_button, scale, CANCEL_ACTION_COLOR);
        cancel_button.hide();
        cancel_button.set_callback(move |_| {
            sender.send(Message::CancelCountdown);
        });

        countdown_row.fixed(&cancel_button, scale.px(CANCEL_BUTTON_WIDTH));
        countdown_row.end();
        layout.fixed(&countdown_row, scale.px(COUNTDOWN_ROW_HEIGHT));

        let mut action_row = group::Flex::default().row();
        action_row.set_spacing(scale.px(8));

        let mut launch_button = button::Button::default();
        style_action_button(&mut launch_button, scale, PRIMARY_ACTION_COLOR);
        launch_button.set_callback(move |_| {
            sender.send(Message::LaunchSelected);
        });

        let mut default_button = button::Button::default();
        style_action_button(&mut default_button, scale, SECONDARY_ACTION_COLOR);
        default_button.set_callback(move |_| {
            sender.send(Message::SetDefaultSelected);
        });

        action_row.end();
        layout.fixed(&action_row, scale.px(ACTION_ROW_HEIGHT));

        let mut ui = Self {
            buttons,
            summary_label,
            countdown_label,
            launch_button,
            default_button,
            cancel_button,
            selected_region,
            default_region,
            countdown_seconds: None,
        };
        ui.refresh();
        ui
    }

    fn set_selected_region(&mut self, region: Region) {
        self.selected_region = region;
        self.refresh();
    }

    fn set_default_region(&mut self, region: Region) {
        self.default_region = region;
        self.refresh();
    }

    fn show_countdown(&mut self, seconds: i32) {
        self.countdown_seconds = Some(seconds);
        self.refresh();
    }

    fn clear_countdown(&mut self) {
        self.countdown_seconds = None;
        self.refresh();
    }

    fn update_ping(&mut self, region: Region, ping_ms: Option<u32>) {
        if let Some(button) = self
            .buttons
            .iter_mut()
            .find(|button| button.region == region)
        {
            button.ping_ms = ping_ms;
        }

        self.refresh();
    }

    fn refresh(&mut self) {
        for button in &mut self.buttons {
            button.refresh(
                button.region == self.selected_region,
                button.region == self.default_region,
            );
        }

        let ping_ms = self.selected_ping();
        let (summary, summary_color) =
            selection_summary(self.selected_region, self.default_region, ping_ms);
        self.summary_label.set_label(&summary);
        self.summary_label.set_label_color(summary_color);

        match self.countdown_seconds {
            Some(seconds) => {
                self.countdown_label
                    .set_label(&countdown_message(self.selected_region, seconds));
                self.cancel_button.show();
            }
            None => {
                self.countdown_label.set_label("");
                self.cancel_button.hide();
            }
        }

        self.launch_button
            .set_label(&format!("Launch {}", self.selected_region));

        if self.selected_region == self.default_region {
            self.default_button.set_label("Default saved");
            self.default_button.deactivate();
        } else {
            self.default_button
                .set_label(&format!("Set {} as default", self.selected_region));
            self.default_button.activate();
        }
    }

    fn selected_ping(&self) -> Option<u32> {
        self.buttons
            .iter()
            .find(|button| button.region == self.selected_region)
            .and_then(|button| button.ping_ms)
    }
}

struct RegionButton {
    widget: button::Button,
    region: Region,
    ping_ms: Option<u32>,
}

impl RegionButton {
    fn new(region: Region, sender: app::Sender<Message>, scale: UiScale) -> Self {
        let mut widget = button::Button::default();
        style_region_button(&mut widget, scale);
        widget.set_callback(move |_| {
            sender.send(Message::SelectRegion(region));
        });

        Self {
            widget,
            region,
            ping_ms: None,
        }
    }

    fn refresh(&mut self, selected: bool, is_default: bool) {
        self.widget.set_label(&region_button_label(
            self.region,
            self.ping_ms,
            selected,
            is_default,
        ));
        self.widget
            .set_color(region_button_color(selected, is_default));
        self.widget.redraw();
    }
}

struct SelectionState {
    selected_region: Region,
    default_region: Region,
}

impl SelectionState {
    fn new(default_region: Region) -> Self {
        Self {
            selected_region: default_region,
            default_region,
        }
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
    SelectRegion(Region),
    LaunchSelected,
    AutoLaunch,
    Countdown(i32),
    CancelCountdown,
    SetDefaultSelected,
    PingResult(Region, Option<u32>),
}

#[cfg(test)]
mod app_tests {
    use super::{countdown_message, ping_presentation, region_button_label, selection_summary};
    use crate::domain::Region;

    #[test]
    fn region_button_label_should_show_selected_default_state() {
        let label = region_button_label(Region::Europe, Some(82), true, true);
        assert_eq!(label, "🇪🇺 Europe\nSelected • Default region • Good • 82 ms");
    }

    #[test]
    fn selection_summary_should_mark_default_region() {
        let (summary, _) = selection_summary(Region::Asia, Region::Asia, Some(74));
        assert_eq!(
            summary,
            "Selected: Asia • Excellent • 74 ms • Default region"
        );
    }

    #[test]
    fn countdown_message_should_reference_selected_region() {
        assert_eq!(
            countdown_message(Region::Americas, 3),
            "Auto-launching Americas in 3s"
        );
    }

    #[test]
    fn ping_presentation_should_return_muted_state_when_unavailable() {
        let ping = ping_presentation(None);
        assert_eq!(ping.description, "Measuring ping");
    }
}
