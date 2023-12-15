#![feature(exact_size_is_empty)]
#![feature(slice_first_last_chunk)]

use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use eframe::{App, CreationContext, NativeOptions};
use egui::{
    text::LayoutJob, Align, Button, CentralPanel, Color32, Context, Frame, Id, Label, Layout, Pos2,
    Sense, Stroke, TextEdit, TextFormat, TextStyle, Vec2, Vec2b, WidgetText, Window, OpenUrl,
};
use egui_extras::{Column, TableBuilder};
use egui_plot::{Line, Plot, PlotPoints};
use itertools::Itertools;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug)]
pub enum Pong {
    Success(Duration),
    Failure,
}

#[derive(Serialize, Deserialize)]
pub struct PingWindow {
    origin: Option<Pos2>,
    hostname: String,
    address: String,
    group: usize,
    scratchpad: String,

    #[serde(skip)]
    #[serde(default = "default_now")]
    ctime: Instant,

    #[serde(skip)]
    #[serde(default = "default_true")]
    open: bool,

    #[serde(skip)]
    scanning: bool,

    #[serde(skip)]
    show_plot: bool,

    #[serde(skip)]
    show_scratchpad: bool,

    #[serde(skip)]
    success: Option<bool>,

    #[serde(skip)]
    history: Vec<(DateTime<Utc>, Pong)>,

    #[serde(skip)]
    #[serde(default = "default_now")]
    last_ping: Instant,
}

impl PingWindow {
    pub fn empty(origin: Option<Pos2>) -> Self {
        Self {
            origin,
            hostname: "localhost (v4)".into(),
            address: "127.0.0.1".into(),
            scratchpad: String::new(),
            group: 0,
            ctime: Instant::now(),
            open: true,
            scanning: false,
            show_plot: false,
            show_scratchpad: false,
            success: None,
            history: vec![],
            last_ping: Instant::now(),
        }
    }

    pub fn new(
        hostname: impl Into<String>,
        address: impl Into<String>,
        origin: Option<Pos2>,
    ) -> Self {
        Self {
            origin,
            hostname: hostname.into(),
            address: address.into(),
            scratchpad: String::new(),
            group: 0,
            ctime: Instant::now(),
            open: true,
            scanning: false,
            show_plot: false,
            show_scratchpad: false,
            success: None,
            history: vec![],
            last_ping: Instant::now(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct PingApp {
    windows: Vec<PingWindow>,
}

impl PingApp {
    fn new(cc: &CreationContext<'_>) -> Self {
        PingApp::default()
    }
}

impl Default for PingApp {
    fn default() -> Self {
        let windows = vec![
            PingWindow::new("localhost (v4)", "127.0.0.1", None),
            PingWindow::new("localhost (v6)", "::1", None),
            PingWindow::new("Google DNS", "8.8.8.8", None),
        ];

        Self { windows }
    }
}

const PLOT_LEN: usize = 20;

const NONE: Color32 = Color32::from_rgb(0x81, 0x82, 0x74);
const PASS: Color32 = Color32::from_rgb(0xA1, 0xC2, 0x31);
const FAIL: Color32 = Color32::from_rgb(0xF4, 0x30, 0x2F);

const GROUPS: [Color32; 5] = [
    Color32::from_gray(0x1B),
    Color32::from_rgb(0x4A, 0x42, 0x25),
    Color32::from_rgb(0x25, 0x4A, 0x30),
    Color32::from_rgb(0x25, 0x2D, 0x4A),
    Color32::from_rgb(0x4A, 0x25, 0x3F),
];

impl App for PingApp {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        ctx.style_mut(|style| style.spacing.item_spacing = Vec2::new(8., 6.));

        CentralPanel::default().show(ctx, |ui| {
            let full_rect = ui.available_rect_before_wrap();
            let interactable = ui.interact(full_rect, Id::new("void"), Sense::click());

            if interactable.double_clicked() {
                let origin = interactable.interact_pointer_pos().unwrap_or_default();
                self.windows.push(PingWindow::empty(Some(origin)));
            }
        });

        for win in &mut self.windows {
            if win.scanning
                && (win.success.is_none() || win.last_ping.elapsed() > Duration::from_secs(1))
            {
                let now = Utc::now();
                let pong = do_ping(&win.address);

                win.last_ping = Instant::now();
                win.history.push((now, pong));

                win.success = match pong {
                    Pong::Success(_) => Some(true),
                    Pong::Failure => Some(false),
                };
            }

            let (icon, color) = match (win.scanning, win.success) {
                (false, _) => ("████", NONE),
                (true, None) => ("████", NONE),
                (true, Some(true)) => ("████", PASS),
                (true, Some(false)) => ("████", FAIL),
            };

            let mut job = LayoutJob::default();
            let font_id = TextStyle::Monospace.resolve(&ctx.style());
            let title = [&win.hostname, "Sin título"][win.hostname.is_empty() as usize];

            let title_format = TextFormat {
                font_id,
                italics: win.hostname.is_empty(),
                ..TextFormat::default()
            };

            let icon_format = TextFormat {
                color,
                italics: false,
                ..title_format.clone()
            };

            job.append(icon, 12., icon_format);
            job.append(title, 12., title_format.clone());
            job.append(" ", 12., title_format);

            let frame = Frame {
                fill: GROUPS[win.group].gamma_multiply(0.75),
                ..Frame::window(&ctx.style())
            };

            let mut window = Window::new(job)
                .id(Id::new(win.ctime))
                .default_width(200.)
                .frame(frame)
                .open(&mut win.open);

            if let Some(origin) = win.origin {
                window = window.default_pos(origin);
            }

            window.show(ctx, |ui| {
                let host_input = TextEdit::singleline(&mut win.hostname)
                    .hint_text(WidgetText::italics("Nombre".into()))
                    .desired_width(ui.available_width())
                    .font(TextStyle::Monospace)
                    .cursor_at_end(true);

                let last_addr = win.address.clone();

                let addr_input = TextEdit::singleline(&mut win.address)
                    .hint_text(WidgetText::italics("Direccion".into()))
                    .desired_width(ui.available_width())
                    .font(TextStyle::Monospace)
                    .cursor_at_end(true);

                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        if ui.toggle_value(&mut win.scanning, "📶").clicked() {
                            win.success = None;
                        }

                        ui.toggle_value(&mut win.show_plot, "📈");
                        ui.toggle_value(&mut win.show_scratchpad, " ¶ ");
                    });

                    ui.vertical_centered_justified(|ui| {
                        ui.horizontal(|ui| {
                            for (idx, color) in GROUPS.into_iter().enumerate() {
                                let stroke = Stroke::new(0.5, Color32::BLACK);
                                let button = Button::new("     ").fill(color).stroke(stroke);

                                if ui.add(button).clicked() {
                                    win.group = idx;
                                }
                            }
                        });

                        ui.add(host_input);

                        if ui.add(addr_input).secondary_clicked() {
                            let open_url = OpenUrl {
                                url: format!("http://{}", last_addr),
                                new_tab: true,
                            };

                            ctx.open_url(open_url);
                        }

                        if win.show_plot {
                            let base = win.history.len().saturating_sub(PLOT_LEN);

                            let groups = win.history[base..].iter().enumerate().group_by(
                                |(_, (_, pong))| match pong {
                                    Pong::Failure => false,
                                    Pong::Success(_) => true,
                                },
                            );

                            let mut lines = vec![];

                            for (success, group) in groups.into_iter() {
                                if !success {
                                    continue;
                                }

                                let samples = group
                                    .map(|(idx, (_, pong))| {
                                        let y = match pong {
                                            Pong::Failure => unreachable!(),
                                            Pong::Success(duration) => duration.as_secs_f64(),
                                        };

                                        [idx as f64, y]
                                    })
                                    .collect::<PlotPoints>();

                                let line = Line::new(samples).fill(0.).color(PASS);
                                lines.push(line);
                            }

                            Plot::new("ping")
                                .show_axes(false)
                                .auto_bounds_y()
                                .include_x(0.)
                                .include_x(PLOT_LEN as f64 - 1.)
                                .allow_drag(Vec2b::FALSE)
                                .reset()
                                .label_formatter(|_, sample| {
                                    let sign = ["", "-"][(sample.y < 0.) as usize];
                                    let secs = sample.y.abs();
                                    let duration = Duration::from_secs_f64(secs);
                                    format!("{}{:?}", sign, duration)
                                })
                                .show(ui, |ui| {
                                    for line in lines {
                                        ui.line(line)
                                    }
                                });
                        } else {
                            // TableBuilder::new(ui)
                            //     .striped(true)
                            //     .column(Column::auto())
                            //     .resizable(true)
                            //     .body(|body| {
                            //         body.rows(24., win.history.len(), |idx, mut row| {
                            //             let (instant, pong) = &win.history[idx];
                            //             let instant = instant.format("%H:%M:%S").to_string();

                            //             let pong = match pong {
                            //                 Pong::Failure => String::from("Unreachable"),
                            //                 Pong::Success(duration) => format!("{:?}", duration),
                            //             };

                            //             row.col(|ui| {
                            //                 ui.add(Label::new(instant).wrap(false));
                            //             });
                            //         })
                            //     });
                        }

                        if win.show_scratchpad {
                            let scratch_input = TextEdit::multiline(&mut win.scratchpad)
                                .font(TextStyle::Monospace)
                                .hint_text(WidgetText::italics("Anotaciones".into()));

                            ui.add(scratch_input);
                        }
                    });
                });
            });
        }

        self.windows.retain(|win| win.open);
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}

fn default_true() -> bool {
    true
}

fn default_now() -> Instant {
    Instant::now()
}

fn do_ping(addr: &str) -> Pong {
    let Ok(lookup) = dns_lookup::lookup_host(addr) else {
        return Pong::Failure;
    };

    let Some(ip) = lookup.first() else {
        return Pong::Failure;
    };

    let pong = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(surge_ping::ping(*ip, &[]));

    match pong {
        Ok((_, duration)) => Pong::Success(duration),
        Err(_) => Pong::Failure,
    }
}

fn main() {
    let _ = eframe::run_native(
        "PingA",
        NativeOptions::default(),
        Box::new(|cc| Box::new(PingApp::new(cc))),
    );
}
