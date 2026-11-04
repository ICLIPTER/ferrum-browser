//! The application state and UI.
//!
//! egui is an *immediate mode* GUI: there are no retained widget objects.
//! `BrowserApp::ui` is called once per frame and rebuilds the whole interface
//! from the state stored in this struct. That state is the browser's memory:
//! the address bar text, the current page, and the history stack.

use std::f32::consts::PI;

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, RichText, Sense, Shape, Stroke,
    TextStyle, Vec2,
};
use url::Url;

use crate::url_input::{AddressError, parse_address};

// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------

const BG: Color32 = Color32::from_rgb(14, 17, 22);
const SURFACE: Color32 = Color32::from_rgb(21, 26, 34);
const SURFACE_HOVER: Color32 = Color32::from_rgb(32, 40, 52);
const BORDER: Color32 = Color32::from_rgb(38, 46, 58);
const TEXT: Color32 = Color32::from_rgb(230, 237, 243);
const MUTED: Color32 = Color32::from_rgb(139, 152, 169);
const ACCENT: Color32 = Color32::from_rgb(91, 157, 255);
const DANGER: Color32 = Color32::from_rgb(255, 107, 107);

const SIDE_PADDING: f32 = 14.0;
const TOOLBAR_HEIGHT: f32 = 34.0;
const GO_WIDTH: f32 = 54.0;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Everything the browser remembers between frames.
pub struct BrowserApp {
    /// Text currently in the address bar (may differ from `current` while editing).
    address: String,
    /// The page the browser believes it is showing.
    current: Option<Url>,
    /// Visited addresses, oldest first.
    history: Vec<Url>,
    /// Index into `history` of the current entry.
    cursor: usize,
    /// Human-readable status shown under the toolbar.
    status: String,
    /// Validation error for the last address-bar submission.
    error: Option<AddressError>,
    /// Set to request keyboard focus on the address bar next frame.
    focus_address: bool,
    /// How many times a page has been opened or reloaded.
    loads: u64,
}

impl Default for BrowserApp {
    fn default() -> Self {
        Self {
            address: String::new(),
            current: None,
            history: Vec::new(),
            cursor: 0,
            status: "Ready — type an address to begin".to_owned(),
            error: None,
            focus_address: true,
            loads: 0,
        }
    }
}

impl BrowserApp {
    /// Called once by eframe before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_global_style(build_style());
        Self::default()
    }

    // -- Navigation --------------------------------------------------------

    fn can_go_back(&self) -> bool {
        !self.history.is_empty() && self.cursor > 0
    }

    fn can_go_forward(&self) -> bool {
        self.cursor + 1 < self.history.len()
    }

    /// Validate the address bar text and navigate if it is usable.
    fn submit_address(&mut self) {
        match parse_address(&self.address) {
            Ok(url) => self.open(url),
            Err(err) => {
                self.status = err.to_string();
                self.error = Some(err);
            }
        }
    }

    /// Load a URL and record it in history, dropping any forward entries.
    fn open(&mut self, url: Url) {
        self.history.truncate(self.cursor + 1);
        self.history.push(url.clone());
        self.cursor = self.history.len() - 1;
        self.loads += 1;

        self.address = url.to_string();
        self.status = format!("Loaded {url}");
        self.error = None;
        self.current = Some(url);
        self.focus_address = false;
    }

    fn open_str(&mut self, raw: &str) {
        self.address = raw.to_owned();
        self.submit_address();
    }

    fn go_back(&mut self) {
        if let Some(index) = self.cursor.checked_sub(1) {
            self.visit_history(index);
        }
    }

    fn go_forward(&mut self) {
        if self.cursor + 1 < self.history.len() {
            self.visit_history(self.cursor + 1);
        }
    }

    fn visit_history(&mut self, index: usize) {
        let Some(url) = self.history.get(index).cloned() else {
            return;
        };

        self.cursor = index;
        self.loads += 1;
        self.address = url.to_string();
        self.status = format!("Loaded {url}");
        self.error = None;
        self.current = Some(url);
    }

    fn reload(&mut self) {
        match self.current.clone() {
            Some(url) => {
                self.loads += 1;
                self.status = format!("Reloaded {url}");
            }
            None => self.status = "Nothing to reload yet".to_owned(),
        }
    }

    // -- UI ----------------------------------------------------------------

    /// Global keyboard shortcuts, handled before any widget is drawn.
    fn handle_shortcuts(&mut self, ui: &egui::Ui) {
        let (focus_bar, back, forward, reload) = ui.input(|input| {
            (
                input.modifiers.command && input.key_pressed(egui::Key::L),
                input.modifiers.alt && input.key_pressed(egui::Key::ArrowLeft),
                input.modifiers.alt && input.key_pressed(egui::Key::ArrowRight),
                input.key_pressed(egui::Key::F5),
            )
        });

        if focus_bar {
            self.focus_address = true;
        }
        if back {
            self.go_back();
        }
        if forward {
            self.go_forward();
        }
        if reload {
            self.reload();
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::NONE
            .fill(SURFACE)
            .inner_margin(Margin::symmetric(SIDE_PADDING as i8, 9))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;

                    if nav_button(ui, NavIcon::Back, self.can_go_back(), "Back  (Alt+Left)")
                        .clicked()
                    {
                        self.go_back();
                    }
                    if nav_button(
                        ui,
                        NavIcon::Forward,
                        self.can_go_forward(),
                        "Forward  (Alt+Right)",
                    )
                    .clicked()
                    {
                        self.go_forward();
                    }
                    if nav_button(ui, NavIcon::Reload, self.current.is_some(), "Reload  (F5)")
                        .clicked()
                    {
                        self.reload();
                    }

                    let spacing = ui.spacing().item_spacing.x;
                    let address_width = (ui.available_width() - GO_WIDTH - spacing).max(160.0);

                    let response = ui.add_sized(
                        [address_width, TOOLBAR_HEIGHT],
                        egui::TextEdit::singleline(&mut self.address)
                            .hint_text("Type an address — for example example.com")
                            .font(TextStyle::Body),
                    );

                    if std::mem::take(&mut self.focus_address) {
                        response.request_focus();
                        // Selecting the whole address (as browsers do) means
                        // Ctrl+L followed by typing replaces instead of appends.
                        let mut state =
                            egui::TextEdit::load_state(ui.ctx(), response.id).unwrap_or_default();
                        state.cursor.set_char_range(Some(
                            egui::text::CCursorRange::select_all_str(&self.address),
                        ));
                        egui::TextEdit::store_state(ui.ctx(), response.id, state);
                    }

                    // A single-line edit surrenders focus when Enter is pressed.
                    let submitted =
                        response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                    let go_clicked = ui
                        .add_sized(
                            [GO_WIDTH, TOOLBAR_HEIGHT],
                            egui::Button::new(RichText::new("Go").color(TEXT)),
                        )
                        .clicked();

                    if submitted || go_clicked {
                        self.submit_address();
                    }
                });
            });

        ui.add(egui::Separator::default().spacing(0.0));
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(SIDE_PADDING);
            match &self.error {
                Some(err) => {
                    ui.label(
                        RichText::new(format!("Error — {err}"))
                            .small()
                            .color(DANGER),
                    );
                }
                None => {
                    ui.label(RichText::new(&self.status).small().color(MUTED));
                }
            }
        });
        ui.add_space(4.0);
    }

    fn content(&mut self, ui: &mut egui::Ui) {
        // Clone so the closure can borrow `self` mutably while viewing the URL.
        let current = self.current.clone();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(30.0);
                match current {
                    None => self.welcome(ui),
                    Some(url) => self.page_view(ui, &url),
                }
                ui.add_space(30.0);
            });
    }

    fn welcome(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("Ferrum Browser")
                    .size(34.0)
                    .strong()
                    .color(TEXT),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "Phase 1 · Foundation — window, address bar, navigation, URL validation",
                )
                .color(MUTED),
            );
            ui.add_space(28.0);

            egui::Frame::NONE
                .fill(SURFACE)
                .stroke(Stroke::new(1.0, BORDER))
                .corner_radius(14.0)
                .inner_margin(Margin::same(22))
                .show(ui, |ui| {
                    ui.set_max_width(520.0);

                    ui.label(RichText::new("Working in this build").strong().color(TEXT));
                    ui.add_space(10.0);
                    bullet(ui, "Native window opened by eframe");
                    bullet(ui, "Address bar that validates and normalises URLs");
                    bullet(ui, "Back / forward history and reload");
                    bullet(ui, "Shortcuts: Ctrl+L, Alt+Left / Alt+Right, F5");

                    ui.add_space(14.0);
                    ui.label(
                        RichText::new(
                            "Page rendering arrives in a later phase. For now Ferrum shows the \
                             parsed address so the navigation layer can be tested on its own.",
                        )
                        .small()
                        .color(MUTED),
                    );

                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Try:").small().color(MUTED));
                        for (label, target) in [
                            ("docs.rs", "https://docs.rs"),
                            ("egui", "https://www.egui.rs"),
                            ("github.com", "https://github.com"),
                        ] {
                            if ui.link(label).clicked() {
                                self.open_str(target);
                            }
                        }
                    });
                });
        });
    }

    fn page_view(&mut self, ui: &mut egui::Ui, url: &Url) {
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(url.host_str().unwrap_or(url.as_str()))
                    .size(28.0)
                    .strong()
                    .color(TEXT),
            );
            ui.add_space(2.0);
            ui.label(RichText::new(url.as_str()).monospace().color(MUTED));
            ui.add_space(24.0);

            egui::Frame::NONE
                .fill(SURFACE)
                .stroke(Stroke::new(1.0, BORDER))
                .corner_radius(14.0)
                .inner_margin(Margin::same(20))
                .show(ui, |ui| {
                    ui.set_max_width(520.0);

                    ui.label(RichText::new("Parsed address").strong().color(TEXT));
                    ui.add_space(10.0);

                    egui::Grid::new("address_parts")
                        .num_columns(2)
                        .spacing([24.0, 6.0])
                        .show(ui, |ui| {
                            part(ui, "Scheme", url.scheme());
                            part(ui, "Host", url.host_str().unwrap_or("—"));
                            part(
                                ui,
                                "Port",
                                &url.port_or_known_default()
                                    .map(|port| port.to_string())
                                    .unwrap_or_else(|| "default".to_owned()),
                            );
                            part(
                                ui,
                                "Path",
                                if url.path().is_empty() {
                                    "/"
                                } else {
                                    url.path()
                                },
                            );
                            part(ui, "Query", url.query().unwrap_or("—"));
                            part(ui, "Fragment", url.fragment().unwrap_or("—"));
                        });

                    ui.add_space(14.0);
                    ui.label(
                        RichText::new(
                            "Static preview — the rendering engine lands in a later phase.",
                        )
                        .small()
                        .color(MUTED),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(format!("Load count: {}", self.loads))
                            .small()
                            .color(MUTED),
                    );
                });
        });
    }
}

impl eframe::App for BrowserApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        BG.to_normalized_gamma_f32()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_shortcuts(ui);
        self.toolbar(ui);
        self.status_bar(ui);
        self.content(ui);
    }
}

// ---------------------------------------------------------------------------
// Small UI helpers
// ---------------------------------------------------------------------------

fn build_style() -> egui::Style {
    let mut style = egui::Style::default();
    let mut visuals = egui::Visuals::dark();

    visuals.panel_fill = BG;
    visuals.window_fill = SURFACE;
    visuals.extreme_bg_color = Color32::from_rgb(10, 13, 18);
    visuals.override_text_color = Some(TEXT);
    visuals.hyperlink_color = ACCENT;
    visuals.selection.bg_fill = Color32::from_rgb(35, 74, 130);
    visuals.widgets.inactive.bg_fill = SURFACE;
    visuals.widgets.inactive.weak_bg_fill = SURFACE;
    visuals.widgets.hovered.bg_fill = SURFACE_HOVER;
    visuals.widgets.hovered.weak_bg_fill = SURFACE_HOVER;
    visuals.widgets.active.bg_fill = SURFACE_HOVER;
    visuals.widgets.active.weak_bg_fill = SURFACE_HOVER;
    style.visuals = visuals;

    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(10.0, 5.0);
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(26.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(15.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(15.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();

    style
}

/// Navigation icons are painted as vectors instead of glyphs: the default
/// font has no arrow characters, so `"←"` would render as a tofu box.
#[derive(Clone, Copy)]
enum NavIcon {
    Back,
    Forward,
    Reload,
}

fn nav_button(ui: &mut egui::Ui, icon: NavIcon, enabled: bool, tooltip: &str) -> egui::Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(34.0, TOOLBAR_HEIGHT), sense);

    let visuals = if enabled {
        *ui.style().interact(&response)
    } else {
        ui.visuals().widgets.noninteractive
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(8), visuals.weak_bg_fill);

    let color = if enabled {
        TEXT
    } else {
        Color32::from_rgb(78, 88, 102)
    };
    let stroke = Stroke::new(1.8, color);
    let center = rect.center();
    let reach = 5.0;

    match icon {
        NavIcon::Back => {
            let tip = center + Vec2::new(-reach, 0.0);
            ui.painter()
                .line_segment([tip, center + Vec2::new(reach, 0.0)], stroke);
            ui.painter()
                .line_segment([tip, tip + Vec2::new(4.5, -4.5)], stroke);
            ui.painter()
                .line_segment([tip, tip + Vec2::new(4.5, 4.5)], stroke);
        }
        NavIcon::Forward => {
            let tip = center + Vec2::new(reach, 0.0);
            ui.painter()
                .line_segment([tip, center + Vec2::new(-reach, 0.0)], stroke);
            ui.painter()
                .line_segment([tip, tip + Vec2::new(-4.5, -4.5)], stroke);
            ui.painter()
                .line_segment([tip, tip + Vec2::new(-4.5, 4.5)], stroke);
        }
        NavIcon::Reload => {
            let radius = 5.5;
            let start = -0.35 * PI;
            let end = 1.15 * PI;
            let steps = 24;

            let points: Vec<_> = (0..=steps)
                .map(|i| {
                    let angle = start + (end - start) * (i as f32 / steps as f32);
                    center + Vec2::angled(angle) * radius
                })
                .collect();
            ui.painter().add(Shape::line(points, stroke));

            // Arrow head at the end of the arc.
            let tip = center + Vec2::angled(end) * radius;
            let tangent = Vec2::new(-end.sin(), end.cos());
            let normal = Vec2::new(-tangent.y, tangent.x);
            let head = 3.8;
            ui.painter().add(Shape::convex_polygon(
                vec![
                    tip,
                    tip - tangent * head + normal * head * 0.8,
                    tip - tangent * head - normal * head * 0.8,
                ],
                color,
                Stroke::NONE,
            ));
        }
    }

    response.on_hover_text(tooltip)
}

fn bullet(ui: &mut egui::Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("•").color(ACCENT));
        ui.label(RichText::new(text).color(TEXT));
    });
}

fn part(ui: &mut egui::Ui, key: &str, value: &str) {
    ui.label(RichText::new(key).small().color(MUTED));
    ui.label(RichText::new(value).monospace().color(TEXT));
    ui.end_row();
}
