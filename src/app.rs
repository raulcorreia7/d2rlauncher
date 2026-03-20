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

const WINDOW_COLOR: Color = Color::from_hex(0x0b1018);
const IDLE_CARD_COLOR: Color = Color::from_hex(0x151e2c);
const SELECTED_CARD_COLOR: Color = Color::from_hex(0x1a2638);
const DEFAULT_CARD_COLOR: Color = Color::from_hex(0x172130);
const PRIMARY_ACTION_COLOR: Color = Color::from_hex(0xb98532);
const SECONDARY_ACTION_COLOR: Color = Color::from_hex(0x1a2232);
const CANCEL_ACTION_COLOR: Color = Color::from_hex(0x2a3345);
const TITLE_TEXT_COLOR: Color = Color::from_hex(0xf2d4a0);
const BODY_TEXT_COLOR: Color = Color::from_hex(0xe8e2d6);
const MUTED_TEXT_COLOR: Color = Color::from_hex(0x9da6b5);
const BADGE_TEXT_COLOR: Color = Color::from_hex(0xf4f0e8);
const COUNTDOWN_TEXT_COLOR: Color = Color::from_hex(0xf0c56f);
const SELECTED_ACCENT_COLOR: Color = Color::from_hex(0xd0a15c);
const DEFAULT_ACCENT_COLOR: Color = Color::from_hex(0x6b7b93);
const IDLE_ACCENT_COLOR: Color = Color::from_hex(0x273246);

const COUNTDOWN_SECONDS: i32 = 5;
const PING_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

const TITLE_HEIGHT: i32 = 24;
const SUBTITLE_HEIGHT: i32 = 16;
const REGION_CARD_HEIGHT: i32 = 64;
const ACTION_ROW_HEIGHT: i32 = 40;
const CARD_ACCENT_WIDTH: i32 = 5;
const PING_BADGE_WIDTH: i32 = 70;
const FAVORITE_BUTTON_WIDTH: i32 = 72;
const LAYOUT_MARGIN: i32 = 16;
const LAYOUT_SPACING: i32 = 10;

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
        Message::SecondaryAction if countdown.borrow().is_cancelled() => {
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
        Message::SecondaryAction => {
            cancel_countdown(countdown, ui);
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
}

fn ping_presentation(ping_ms: Option<u32>) -> PingPresentation {
    match ping_ms {
        Some(ms) if ms < 70 => PingPresentation {
            badge_color: Color::from_hex(0x1d3a31),
        },
        Some(ms) if ms < 180 => PingPresentation {
            badge_color: Color::from_hex(0x514315),
        },
        Some(_) => PingPresentation {
            badge_color: Color::from_hex(0x5a2525),
        },
        None => PingPresentation {
            badge_color: Color::from_hex(0x24324b),
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

fn region_status_label(selected: bool, is_default: bool) -> String {
    match (selected, is_default) {
        (true, _) => String::new(),
        (false, true) => "Favorite".to_string(),
        (false, false) => String::new(),
    }
}

fn ping_badge_label(ping_ms: Option<u32>) -> String {
    match ping_ms {
        Some(ms) => format!("{ms} ms"),
        None => "-- ms".to_string(),
    }
}

fn launch_button_label(region: Region, countdown_seconds: Option<i32>) -> String {
    match countdown_seconds {
        Some(seconds) => format!("Auto-launch in {seconds}s"),
        None => format!("Launch {region}"),
    }
}

fn region_title_label(region: Region, is_default: bool) -> String {
    if is_default {
        format!("{} {}  ★", region.flag(), region)
    } else {
        format!("{} {}", region.flag(), region)
    }
}

fn style_title(frame: &mut frame::Frame, scale: UiScale) {
    frame.set_label_size(scale.px(17));
    frame.set_label_color(TITLE_TEXT_COLOR);
    frame.set_label_font(Font::HelveticaBold);
    frame.set_align(Align::Left | Align::Inside);
}

fn style_subtitle(frame: &mut frame::Frame, scale: UiScale) {
    frame.set_label_size(scale.px(10));
    frame.set_label_color(MUTED_TEXT_COLOR);
    frame.set_label_font(Font::HelveticaItalic);
    frame.set_align(Align::Left | Align::Inside);
}

fn style_action_button(btn: &mut button::Button, scale: UiScale, color: Color) {
    btn.set_frame(FrameType::RoundedBox);
    btn.set_color(color);
    btn.set_label_color(Color::White);
    btn.set_label_size(scale.px(11));
    btn.set_label_font(Font::HelveticaBold);
}

fn style_card_title(frame: &mut frame::Frame, scale: UiScale) {
    frame.set_label_size(scale.px(13));
    frame.set_label_color(BODY_TEXT_COLOR);
    frame.set_label_font(Font::HelveticaBold);
    frame.set_align(Align::Left | Align::Inside);
}

fn style_card_status(frame: &mut frame::Frame, scale: UiScale) {
    frame.set_label_size(scale.px(10));
    frame.set_label_color(MUTED_TEXT_COLOR);
    frame.set_label_font(Font::Helvetica);
    frame.set_align(Align::Left | Align::Inside);
}

fn style_ping_badge(frame: &mut frame::Frame, scale: UiScale) {
    frame.set_frame(FrameType::RoundedBox);
    frame.set_label_size(scale.px(9));
    frame.set_label_color(BADGE_TEXT_COLOR);
    frame.set_label_font(Font::HelveticaBold);
    frame.set_align(Align::Center | Align::Inside);
}

struct Ui {
    cards: Vec<RegionCard>,
    launch_button: button::Button,
    default_button: button::Button,
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

        let mut subtitle = frame::Frame::default().with_label("Select a region, then launch.");
        style_subtitle(&mut subtitle, scale);
        layout.fixed(&subtitle, scale.px(SUBTITLE_HEIGHT));

        let cards = Region::ALL
            .iter()
            .map(|&region| RegionCard::new(region, sender, scale))
            .collect::<Vec<_>>();

        for card in &cards {
            layout.fixed(&card.root, scale.px(REGION_CARD_HEIGHT));
        }

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
            default_sender.send(Message::SecondaryAction);
        });

        action_row.fixed(&default_button, scale.px(FAVORITE_BUTTON_WIDTH));
        action_row.end();
        layout.fixed(&action_row, scale.px(ACTION_ROW_HEIGHT));

        let mut ui = Self {
            cards,
            launch_button,
            default_button,
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

        self.launch_button.set_label(&launch_button_label(
            self.selected_region,
            self.countdown_seconds,
        ));

        match self.countdown_seconds {
            Some(_) => {
                self.launch_button.set_color(Color::from_hex(0x7e6031));
                self.launch_button.set_label_color(COUNTDOWN_TEXT_COLOR);
                self.default_button.set_label("Cancel");
                self.default_button.set_color(CANCEL_ACTION_COLOR);
                self.default_button.set_label_color(Color::White);
            }
            None if self.selected_region == self.default_region => {
                self.launch_button.set_color(PRIMARY_ACTION_COLOR);
                self.launch_button.set_label_color(Color::White);
                self.default_button.set_label("★");
                self.default_button.set_color(Color::from_hex(0x243143));
                self.default_button.set_label_color(TITLE_TEXT_COLOR);
            }
            None => {
                self.launch_button.set_color(PRIMARY_ACTION_COLOR);
                self.launch_button.set_label_color(Color::White);
                self.default_button.set_label("☆");
                self.default_button.set_color(SECONDARY_ACTION_COLOR);
                self.default_button.set_label_color(BODY_TEXT_COLOR);
            }
        }
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
            .set_label(&region_title_label(self.region, is_default));
        self.status
            .set_label(&region_status_label(selected, is_default));

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
    SecondaryAction,
    PingResult(Region, Option<u32>),
}

#[cfg(test)]
mod app_tests {
    use super::{launch_button_label, ping_badge_label, ping_presentation, region_status_label};
    use crate::domain::Region;
    use fltk::enums::Color;

    #[test]
    fn region_status_label_should_hide_selected_state_copy() {
        let label = region_status_label(true, true);
        assert_eq!(label, "");
    }

    #[test]
    fn launch_button_label_should_switch_for_countdown() {
        assert_eq!(launch_button_label(Region::Asia, None), "Launch Asia");
        assert_eq!(
            launch_button_label(Region::Asia, Some(4)),
            "Auto-launch in 4s"
        );
    }

    #[test]
    fn ping_presentation_should_return_neutral_badge_when_unavailable() {
        let ping = ping_presentation(None);
        assert_eq!(ping.badge_color, Color::from_hex(0x24324b));
    }

    #[test]
    fn ping_badge_label_should_show_placeholder_when_ping_is_unknown() {
        assert_eq!(ping_badge_label(None), "-- ms");
    }

    #[test]
    fn ping_presentation_should_keep_mid_latency_in_good_range() {
        let ping = ping_presentation(Some(154));
        assert_eq!(ping.badge_color, Color::from_hex(0x514315));
    }
}
