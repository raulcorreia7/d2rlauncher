use fltk::{
    app, button,
    enums::{Align, Color, Event, Font, FrameType},
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

const WINDOW_COLOR: Color = Color::from_hex(0x090807);
const SURFACE_COLOR: Color = Color::from_hex(0x15110d);
const IDLE_CARD_COLOR: Color = Color::from_hex(0x1a1510);
const SELECTED_CARD_COLOR: Color = Color::from_hex(0x201811);
const DEFAULT_CARD_COLOR: Color = Color::from_hex(0x19140f);
const PRIMARY_ACTION_COLOR: Color = Color::from_hex(0xa66a19);
const SECONDARY_ACTION_COLOR: Color = Color::from_hex(0x2b2218);
const CANCEL_ACTION_COLOR: Color = Color::from_hex(0x403427);
const TITLE_TEXT_COLOR: Color = Color::from_hex(0xe3c48a);
const BODY_TEXT_COLOR: Color = Color::from_hex(0xe6dccb);
const MUTED_TEXT_COLOR: Color = Color::from_hex(0xb0a08a);
const BADGE_TEXT_COLOR: Color = Color::from_hex(0xe2e8f0);
const COUNTDOWN_TEXT_COLOR: Color = Color::from_hex(0xfbbf24);
const SELECTED_ACCENT_COLOR: Color = Color::from_hex(0xd29a3a);
const DEFAULT_ACCENT_COLOR: Color = Color::from_hex(0x7a6843);
const IDLE_ACCENT_COLOR: Color = Color::from_hex(0x463729);

const COUNTDOWN_SECONDS: i32 = 5;
const PING_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

const TITLE_HEIGHT: i32 = 24;
const SUBTITLE_HEIGHT: i32 = 16;
const STATUS_ROW_HEIGHT: i32 = 42;
const REGION_CARD_HEIGHT: i32 = 62;
const ACTION_ROW_HEIGHT: i32 = 40;
const CARD_ACCENT_WIDTH: i32 = 5;
const PING_BADGE_WIDTH: i32 = 74;
const CANCEL_BUTTON_WIDTH: i32 = 108;
const LAYOUT_MARGIN: i32 = 14;
const LAYOUT_SPACING: i32 = 8;
const PANEL_PADDING_X: i32 = 12;
const PANEL_PADDING_Y: i32 = 8;

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
    bind_window_click_cancel(&mut wind, sender);

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

fn bind_window_click_cancel(wind: &mut window::Window, sender: app::Sender<Message>) {
    wind.handle(move |_wind, event| {
        if matches!(event, Event::Push) {
            sender.send(Message::CancelCountdown);
        }

        false
    });
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
        Message::Countdown(seconds) => {
            ui.show_countdown(seconds);
            Ok(None)
        }
        Message::CancelCountdown => {
            cancel_countdown(countdown, ui);
            Ok(None)
        }
        Message::SetDefaultSelected => {
            cancel_countdown(countdown, ui);
            let region = selection.selected_region;
            if selection.default_region == region {
                return Ok(None);
            }

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
        CountdownProgress::Running(seconds) => {
            sender.send(Message::Countdown(seconds));
            schedule_countdown(state.clone(), sender);
        }
        CountdownProgress::Complete => {
            sender.send(Message::AutoLaunch);
        }
    });
}

fn setup_theme() {
    app::background(0x0b, 0x12, 0x20);
    app::background2(0x12, 0x1b, 0x2e);
    app::foreground(0xff, 0xff, 0xff);
}

fn create_window(scale: UiScale) -> window::Window {
    let mut wind = window::Window::default()
        .with_size(
            scale.px(constants::WINDOW_WIDTH),
            scaled_window_height(scale),
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
    col.set_margins(
        scale.px(LAYOUT_MARGIN),
        scale.px(LAYOUT_MARGIN),
        scale.px(LAYOUT_MARGIN),
        scale.px(LAYOUT_MARGIN),
    );
    col.set_spacing(scale.px(LAYOUT_SPACING));
    col
}

fn scaled_window_height(scale: UiScale) -> i32 {
    let row_heights = [
        TITLE_HEIGHT,
        SUBTITLE_HEIGHT,
        REGION_CARD_HEIGHT,
        REGION_CARD_HEIGHT,
        REGION_CARD_HEIGHT,
        STATUS_ROW_HEIGHT,
        ACTION_ROW_HEIGHT,
    ];

    let content_height = row_heights.into_iter().sum::<i32>();
    let spacing_height = LAYOUT_SPACING * (row_heights.len() as i32 - 1);
    let margins_height = LAYOUT_MARGIN * 2;

    scale.px(content_height + spacing_height + margins_height)
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
    badge_color: Color,
    summary_color: Color,
    description: &'static str,
}

fn ping_presentation(ping_ms: Option<u32>) -> PingPresentation {
    match ping_ms {
        Some(ms) if ms < 70 => PingPresentation {
            badge_color: Color::from_hex(0x1d3a31),
            summary_color: BODY_TEXT_COLOR,
            description: "Excellent",
        },
        Some(ms) if ms < 180 => PingPresentation {
            badge_color: Color::from_hex(0x514315),
            summary_color: BODY_TEXT_COLOR,
            description: "Good",
        },
        Some(_) => PingPresentation {
            badge_color: Color::from_hex(0x5a2525),
            summary_color: BODY_TEXT_COLOR,
            description: "High ping",
        },
        None => PingPresentation {
            badge_color: Color::from_hex(0x24324b),
            summary_color: BODY_TEXT_COLOR,
            description: "Measuring ping",
        },
    }
}

fn region_card_color(selected: bool, is_default: bool) -> Color {
    match (selected, is_default) {
        (true, _) => SELECTED_CARD_COLOR,
        (false, true) => DEFAULT_CARD_COLOR,
        (false, false) => IDLE_CARD_COLOR,
    }
}

fn region_accent_color(selected: bool, is_default: bool) -> Color {
    match (selected, is_default) {
        (true, _) => SELECTED_ACCENT_COLOR,
        (false, true) => DEFAULT_ACCENT_COLOR,
        (false, false) => IDLE_ACCENT_COLOR,
    }
}

fn region_status_label(selected: bool, is_default: bool, ping_ms: Option<u32>) -> String {
    let state = match (selected, is_default) {
        (true, true) => "Selected • favorite",
        (true, false) => "Selected region",
        (false, true) => "Favorite",
        (false, false) => "Click to select",
    };

    format!("{state} • {}", ping_presentation(ping_ms).description)
}

fn ping_badge_label(ping_ms: Option<u32>) -> String {
    match ping_ms {
        Some(ms) => format!("{ms} ms"),
        None => "-- ms".to_string(),
    }
}

fn ready_message(region: Region, ping_ms: Option<u32>) -> (String, Color) {
    let ping = ping_presentation(ping_ms);
    let label = match ping_ms {
        Some(ms) => format!("Ready to launch {region} • {ms} ms"),
        None => format!("Ready to launch {region}"),
    };

    (label, ping.summary_color)
}

fn countdown_message(region: Region, seconds: i32) -> String {
    format!("Auto-launching {region} in {seconds}s")
}

fn style_title(frame: &mut frame::Frame, scale: UiScale) {
    frame.set_label_size(scale.px(16));
    frame.set_label_color(TITLE_TEXT_COLOR);
    frame.set_label_font(Font::TimesBold);
    frame.set_align(Align::Left | Align::Inside);
}

fn style_subtitle(frame: &mut frame::Frame, scale: UiScale) {
    frame.set_label_size(scale.px(10));
    frame.set_label_color(MUTED_TEXT_COLOR);
    frame.set_label_font(Font::TimesItalic);
    frame.set_align(Align::Left | Align::Inside);
}

fn create_status_panel(scale: UiScale, label_color: Color) -> (group::Flex, frame::Frame) {
    let mut panel = group::Flex::default().row();
    panel.set_frame(FrameType::BorderBox);
    panel.set_color(SURFACE_COLOR);
    panel.set_margins(
        scale.px(PANEL_PADDING_X),
        scale.px(PANEL_PADDING_Y),
        scale.px(PANEL_PADDING_X),
        scale.px(PANEL_PADDING_Y),
    );

    let mut label = frame::Frame::default();
    style_status_label(&mut label, scale, label_color);

    panel.end();
    (panel, label)
}

fn style_status_label(frame: &mut frame::Frame, scale: UiScale, color: Color) {
    frame.set_label_color(color);
    frame.set_label_size(scale.px(10));
    frame.set_label_font(Font::Times);
    frame.set_align(Align::Left | Align::Inside);
}

fn style_action_button(btn: &mut button::Button, scale: UiScale, color: Color) {
    btn.set_frame(FrameType::UpBox);
    btn.set_color(color);
    btn.set_label_color(Color::White);
    btn.set_label_size(scale.px(11));
    btn.set_label_font(Font::TimesBold);
}

fn style_card_title(frame: &mut frame::Frame, scale: UiScale) {
    frame.set_label_size(scale.px(13));
    frame.set_label_color(BODY_TEXT_COLOR);
    frame.set_label_font(Font::TimesBold);
    frame.set_align(Align::Left | Align::Inside);
}

fn style_card_status(frame: &mut frame::Frame, scale: UiScale) {
    frame.set_label_size(scale.px(10));
    frame.set_label_color(MUTED_TEXT_COLOR);
    frame.set_label_font(Font::Times);
    frame.set_align(Align::Left | Align::Inside);
}

fn style_ping_badge(frame: &mut frame::Frame, scale: UiScale) {
    frame.set_frame(FrameType::BorderBox);
    frame.set_label_size(scale.px(9));
    frame.set_label_color(BADGE_TEXT_COLOR);
    frame.set_label_font(Font::TimesBold);
    frame.set_align(Align::Center | Align::Inside);
}

struct Ui {
    cards: Vec<RegionCard>,
    status_label: frame::Frame,
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
        let mut title = frame::Frame::default().with_label("Choose Your Region");
        style_title(&mut title, scale);
        layout.fixed(&title, scale.px(TITLE_HEIGHT));

        let mut subtitle =
            frame::Frame::default().with_label("Choose your gateway into Sanctuary.");
        style_subtitle(&mut subtitle, scale);
        layout.fixed(&subtitle, scale.px(SUBTITLE_HEIGHT));

        let cards = Region::ALL
            .iter()
            .map(|&region| RegionCard::new(region, sender, scale))
            .collect::<Vec<_>>();

        for card in &cards {
            layout.fixed(&card.root, scale.px(REGION_CARD_HEIGHT));
        }

        let mut status_row = group::Flex::default().row();
        status_row.set_spacing(scale.px(8));

        let (_status_panel, status_label) = create_status_panel(scale, Color::White);

        let mut cancel_button = button::Button::default().with_label("Cancel");
        style_action_button(&mut cancel_button, scale, CANCEL_ACTION_COLOR);
        cancel_button.hide();

        let cancel_sender = sender;
        cancel_button.set_callback(move |_| {
            cancel_sender.send(Message::CancelCountdown);
        });

        status_row.fixed(&cancel_button, scale.px(CANCEL_BUTTON_WIDTH));
        status_row.end();
        layout.fixed(&status_row, scale.px(STATUS_ROW_HEIGHT));

        let mut action_row = group::Flex::default().row();
        action_row.set_spacing(scale.px(8));

        let mut launch_button = button::Button::default();
        style_action_button(&mut launch_button, scale, PRIMARY_ACTION_COLOR);

        let launch_sender = sender;
        launch_button.set_callback(move |_| {
            launch_sender.send(Message::LaunchSelected);
        });

        let mut default_button = button::Button::default();
        style_action_button(&mut default_button, scale, SECONDARY_ACTION_COLOR);

        let default_sender = sender;
        default_button.set_callback(move |_| {
            default_sender.send(Message::SetDefaultSelected);
        });

        action_row.end();
        layout.fixed(&action_row, scale.px(ACTION_ROW_HEIGHT));

        let mut ui = Self {
            cards,
            status_label,
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
        if let Some(card) = self.cards.iter_mut().find(|card| card.region == region) {
            card.ping_ms = ping_ms;
        }

        self.refresh();
    }

    fn refresh(&mut self) {
        for card in &mut self.cards {
            card.refresh(
                card.region == self.selected_region,
                card.region == self.default_region,
            );
        }

        let ping_ms = self.selected_ping();

        match self.countdown_seconds {
            Some(seconds) => {
                self.status_label
                    .set_label(&countdown_message(self.selected_region, seconds));
                self.status_label.set_label_color(COUNTDOWN_TEXT_COLOR);
                self.cancel_button.show();
            }
            None => {
                let (status, color) = ready_message(self.selected_region, ping_ms);
                self.status_label.set_label(&status);
                self.status_label.set_label_color(color);
                self.cancel_button.hide();
            }
        }

        self.launch_button
            .set_label(&format!("Launch {}", self.selected_region));

        if self.selected_region == self.default_region {
            self.default_button.set_label("★ Favorite");
            self.default_button.set_color(Color::from_hex(0x3a2d1c));
            self.default_button.set_label_color(TITLE_TEXT_COLOR);
            self.default_button.activate();
        } else {
            self.default_button.set_label("☆ Favorite");
            self.default_button.set_color(SECONDARY_ACTION_COLOR);
            self.default_button.set_label_color(BODY_TEXT_COLOR);
            self.default_button.activate();
        }
    }

    fn selected_ping(&self) -> Option<u32> {
        self.cards
            .iter()
            .find(|card| card.region == self.selected_region)
            .and_then(|card| card.ping_ms)
    }
}

struct RegionCard {
    root: group::Flex,
    accent: frame::Frame,
    title: frame::Frame,
    status: frame::Frame,
    ping_badge: frame::Frame,
    region: Region,
    ping_ms: Option<u32>,
}

impl RegionCard {
    fn new(region: Region, sender: app::Sender<Message>, scale: UiScale) -> Self {
        let mut root = group::Flex::default().row();
        root.set_margins(scale.px(10), scale.px(10), scale.px(14), scale.px(10));
        root.set_spacing(scale.px(12));
        root.set_frame(FrameType::BorderBox);

        let mut accent = frame::Frame::default();
        accent.set_frame(FrameType::FlatBox);

        let mut text_col = group::Flex::default().column();
        text_col.set_spacing(scale.px(2));

        let mut title = frame::Frame::default();
        style_card_title(&mut title, scale);

        let mut status = frame::Frame::default();
        style_card_status(&mut status, scale);

        text_col.end();

        let mut ping_badge = frame::Frame::default();
        style_ping_badge(&mut ping_badge, scale);

        root.fixed(&accent, scale.px(CARD_ACCENT_WIDTH));
        root.fixed(&ping_badge, scale.px(PING_BADGE_WIDTH));
        root.end();

        root.handle(move |_group, event| {
            if matches!(event, Event::Released) && app::event_button() == 1 {
                sender.send(Message::SelectRegion(region));
                true
            } else {
                false
            }
        });

        Self {
            root,
            accent,
            title,
            status,
            ping_badge,
            region,
            ping_ms: None,
        }
    }

    fn refresh(&mut self, selected: bool, is_default: bool) {
        self.root.set_color(region_card_color(selected, is_default));
        self.accent
            .set_color(region_accent_color(selected, is_default));
        self.title
            .set_label(&format!("{} {}", self.region.flag(), self.region));
        self.status
            .set_label(&region_status_label(selected, is_default, self.ping_ms));

        let ping = ping_presentation(self.ping_ms);
        self.ping_badge.set_color(ping.badge_color);
        self.ping_badge.set_label(&ping_badge_label(self.ping_ms));

        self.root.redraw();
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
    use super::{
        countdown_message, ping_badge_label, ping_presentation, ready_message, region_status_label,
    };
    use crate::domain::Region;

    #[test]
    fn region_status_label_should_show_selected_default_state() {
        let label = region_status_label(true, true, Some(82));
        assert_eq!(label, "Selected • favorite • Good");
    }

    #[test]
    fn ready_message_should_focus_on_launch_action() {
        let (summary, _) = ready_message(Region::Asia, Some(74));
        assert_eq!(summary, "Ready to launch Asia • 74 ms");
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

    #[test]
    fn ping_badge_label_should_show_placeholder_when_ping_is_unknown() {
        assert_eq!(ping_badge_label(None), "-- ms");
    }

    #[test]
    fn ping_presentation_should_keep_mid_latency_in_good_range() {
        let ping = ping_presentation(Some(154));
        assert_eq!(ping.description, "Good");
    }
}
