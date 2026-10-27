use crate::{
    demo::{demo_snapshot, reveal},
    platform::{self, Drive},
    theme::*,
};

use diskpie::{
    chart,
    export::csv_field,
    model::{NONE, NodeId, Snapshot, size_label},
    query::{QueryResult, Sort, View},
    tasks::{QueryTask, ScanTask, publish},
};

use eframe::egui::{self, Align2, Color32, FontId, RichText, Stroke, Vec2};

use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering, mpsc},
    time::{Duration, Instant},
};

const PAGE: usize = 200;

pub struct DiskPieApp {
    location: String,
    excludes: String,
    workers: usize,
    scan: Option<ScanTask>,
    snapshot: Option<Arc<Snapshot>>,
    query: QueryTask,
    result: QueryResult,
    view: View,
    folder: NodeId,
    search: String,
    search_changed: Option<Instant>,
    busy: bool,
    types: bool,
    issues: bool,
    message: String,
    picker: Option<mpsc::Receiver<Option<PathBuf>>>,
    demo: bool,
    drives: Vec<Drive>,
    drive_rx: Option<mpsc::Receiver<Vec<Drive>>>,
    sort: Sort,
    descending: bool,
    page: usize,
    export_rx: Option<mpsc::Receiver<Result<(), String>>>,
    completed: String,
}

impl DiskPieApp {
    pub fn new(cc: &eframe::CreationContext<'_>, root: Option<PathBuf>, demo: bool) -> Self {
        apply(&cc.egui_ctx);

        let mut app = Self {
            location: root
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| {
                    if cfg!(windows) {
                        "C:\\".into()
                    } else {
                        "/".into()
                    }
                }),
            excludes: String::new(),
            workers: 4,
            scan: None,
            snapshot: None,
            query: QueryTask::default(),
            result: QueryResult::default(),
            view: View::Folder(0),
            folder: 0,
            search: String::new(),
            search_changed: None,
            busy: false,
            types: false,
            issues: false,
            message: String::new(),
            picker: None,
            demo,
            drives: Vec::new(),
            drive_rx: None,
            sort: Sort::Size,
            descending: true,
            page: 0,
            export_rx: None,
            completed: String::new(),
        };

        if demo {
            app.location = "Demo".into();

            app.snapshot = Some(Arc::new(demo_snapshot()));

            app.completed = "Synthetic preview".into();

            app.drives = vec![Drive {
                path: "Demo".into(),
                total: 256 << 30,
                free: 158 << 30,
            }];

            app.navigate(View::Folder(0));
        } else {
            app.refresh_drives();

            if root.is_some() {
                app.start_scan();
            }
        }

        app
    }

    fn refresh_drives(&mut self) {
        if self.drive_rx.is_some() || self.demo {
            return;
        }

        let (tx, rx) = mpsc::channel();

        self.drive_rx = Some(rx);

        std::thread::spawn(move || {
            let _ = tx.send(platform::drives());
        });
    }

    fn start_scan(&mut self) {
        if self.scan.is_some() || self.location.trim().is_empty() {
            return;
        }

        self.demo = false;

        let excludes = self
            .excludes
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect();

        self.scan = Some(ScanTask::start_excluding(
            PathBuf::from(self.location.trim()),
            self.workers,
            excludes,
        ));

        self.message.clear();
    }

    fn request(&mut self) {
        if let Some(s) = &self.snapshot {
            self.busy = true;

            self.result = QueryResult::default();

            self.query.request_page(
                s.clone(),
                self.view.clone(),
                self.sort,
                self.descending,
                self.page * PAGE,
            );
        }
    }

    fn navigate(&mut self, view: View) {
        if let View::Folder(id) = view {
            self.folder = id;
        }

        self.view = view;

        self.types = false;

        self.issues = false;

        self.page = 0;

        self.request();
    }

    fn open(&mut self, id: NodeId, ui: &egui::Ui, s: &Snapshot) {
        if s.nodes[id as usize].is_dir {
            self.search.clear();

            self.navigate(View::Folder(id));
        } else {
            ui.ctx().copy_text(s.path(id).display().to_string());

            self.message = "Path copied".into();
        }
    }

    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(task) = &self.scan {
            match task.receiver.try_recv() {
                Ok(result) => {
                    match publish(&mut self.snapshot, result, &task.cancel) {
                        Ok(()) => {
                            self.completed = "Completed this session".into();

                            self.message.clear();

                            self.search.clear();

                            self.navigate(View::Folder(0));
                        }

                        Err(e) => self.message = e,
                    }

                    self.scan = None;
                }

                Err(mpsc::TryRecvError::Disconnected) => {
                    self.scan = None;

                    self.message = "Scan worker stopped; previous results kept".into();
                }

                Err(_) => {}
            }
        }

        while let Ok((generation, result)) = self.query.receiver.try_recv() {
            if generation == self.query.current {
                self.result = result;

                self.busy = false;
            }
        }

        if let Some(rx) = &self.drive_rx {
            match rx.try_recv() {
                Ok(d) => {
                    self.drives = d;

                    self.drive_rx = None;
                }

                Err(mpsc::TryRecvError::Disconnected) => self.drive_rx = None,
                Err(_) => {}
            }
        }

        if let Some(rx) = &self.picker {
            match rx.try_recv() {
                Ok(p) => {
                    if let Some(p) = p {
                        self.location = p.display().to_string();
                    }

                    self.picker = None;
                }

                Err(mpsc::TryRecvError::Disconnected) => self.picker = None,
                Err(_) => {}
            }
        }

        if let Some(rx) = &self.export_rx {
            match rx.try_recv() {
                Ok(result) => {
                    self.message = match result {
                        Ok(()) => "Export finished".into(),
                        Err(e) => e,
                    };

                    self.export_rx = None;
                }

                Err(mpsc::TryRecvError::Disconnected) => self.export_rx = None,
                Err(_) => {}
            }
        }

        if self
            .search_changed
            .is_some_and(|t| t.elapsed() >= Duration::from_millis(250))
        {
            self.search_changed = None;

            self.navigate(if self.search.trim().is_empty() {
                View::Folder(self.folder)
            } else {
                View::Search(self.search.trim().into())
            });
        }

        if self.scan.is_some()
            || self.busy
            || self.picker.is_some()
            || self.drive_rx.is_some()
            || self.export_rx.is_some()
            || self.search_changed.is_some()
        {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(12.);

        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(31.), egui::Sense::hover());

            let p = ui.painter();

            p.circle_filled(r.center(), 15., PURPLE);

            let c = r.center();

            p.add(egui::Shape::convex_polygon(
                vec![
                    c,
                    c + Vec2::new(0., -15.),
                    c + Vec2::new(10.6, -10.6),
                    c + Vec2::new(15., 0.),
                ],
                MINT,
                Stroke::NONE,
            ));

            ui.label(
                RichText::new("DiskPie")
                    .size(25.)
                    .family(egui::FontFamily::Name("bold".into())),
            );

            ui.label(RichText::new("2").small().color(MUTED));
        });

        ui.add_space(38.);

        eyebrow(ui, "WORKSPACE");

        ui.add_space(10.);

        let active = if self.issues {
            3
        } else if self.types {
            2
        } else if matches!(self.view, View::Largest | View::Extension(_)) {
            1
        } else {
            0
        };

        for (index, label) in [
            "◫    Folder explorer",
            "↗    Largest files",
            "▦    File types",
            "ⓘ    Skipped locations",
        ]
        .iter()
        .enumerate()
        {
            let selected = index == active;

            let b = egui::Button::new(RichText::new(*label).color(if selected {
                Color32::from_rgb(211, 200, 255)
            } else {
                MUTED
            }))
            .fill(if selected {
                Color32::from_rgb(43, 38, 63)
            } else {
                Color32::TRANSPARENT
            })
            .stroke(Stroke::NONE)
            .corner_radius(8)
            .right_text("");

            if ui.add_sized([ui.available_width(), 44.], b).clicked() {
                self.search.clear();

                self.search_changed = None;

                match index {
                    0 => self.navigate(View::Folder(self.folder)),
                    1 => self.navigate(View::Largest),
                    2 => {
                        self.types = true;

                        self.issues = false;

                        self.page = 0;
                    }

                    _ => {
                        self.issues = true;

                        self.types = false;

                        self.page = 0;
                    }
                }
            }
        }

        ui.add_space(26.);

        ui.horizontal(|ui| {
            eyebrow(ui, "YOUR DRIVES");

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        !self.demo && self.drive_rx.is_none(),
                        egui::Button::new("↻").frame(false),
                    )
                    .on_hover_text("Refresh drives")
                    .clicked()
                {
                    self.refresh_drives();
                }
            });
        });

        ui.add_space(8.);

        let drives = self.drives.clone();

        egui::ScrollArea::vertical()
            .id_salt("drives")
            .max_height((ui.available_height() - 125.).max(60.))
            .show(ui, |ui| {
                for d in drives {
                    let chosen = self.location == d.path;

                    let f = egui::Frame::new()
                        .fill(if chosen {
                            Color32::from_rgb(30, 34, 44)
                        } else {
                            Color32::TRANSPARENT
                        })
                        .corner_radius(8)
                        .inner_margin(10);

                    let r = f
                        .show(ui, |ui| {
                            ui.set_width((ui.available_width() - 2.).max(20.));

                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("▣ {}", d.path))
                                        .family(egui::FontFamily::Name("bold".into())),
                                );

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if d.total > 0 {
                                            muted(ui, size_label(d.total));
                                        }
                                    },
                                );
                            });

                            if d.total > 0 {
                                ui.add(
                                    egui::ProgressBar::new(1. - d.free as f32 / d.total as f32)
                                        .fill(PURPLE)
                                        .desired_height(4.),
                                );

                                muted(ui, format!("{} available", size_label(d.free)));
                            }
                        })
                        .response;

                    if ui
                        .interact(r.rect, ui.id().with(&d.path), egui::Sense::click())
                        .clicked()
                    {
                        self.location = d.path;
                    }

                    ui.add_space(6.);
                }
            });

        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.add_space(8.);

            if ui.small_button("Quit DiskPie").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }

            muted(ui, "Metadata only. Your files stay yours.");

            ui.label(RichText::new("●  Local & private").size(11.).color(MINT));
        });
    }

    fn scan_box(&mut self, ui: &mut egui::Ui) {
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());

            eyebrow(ui, "SCAN LOCATION");

            ui.add_space(5.);

            let scanning = self.scan.is_some();

            ui.horizontal(|ui| {
                ui.label(RichText::new("⌕").size(23.).color(MUTED));

                let width = (ui.available_width() - 220.).max(70.);

                let edit = ui.add_enabled(
                    !scanning,
                    egui::TextEdit::singleline(&mut self.location)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(width)
                        .margin(Vec2::new(12., 12.)),
                );

                if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.start_scan();
                }

                if ui
                    .add_enabled(
                        !scanning && self.picker.is_none(),
                        egui::Button::new("…").min_size(Vec2::new(34., 42.)),
                    )
                    .on_hover_text("Browse for a folder")
                    .clicked()
                {
                    let (tx, rx) = mpsc::channel();

                    self.picker = Some(rx);

                    std::thread::spawn(move || {
                        let _ = tx.send(rfd::FileDialog::new().pick_folder());
                    });
                }

                if scanning {
                    if ui
                        .add_sized(
                            [145., 42.],
                            egui::Button::new("Stop scan").fill(Color32::from_rgb(73, 41, 45)),
                        )
                        .clicked()
                    {
                        self.scan
                            .as_ref()
                            .unwrap()
                            .cancel
                            .store(true, Ordering::Relaxed);
                    }
                } else if ui
                    .add_enabled(
                        !self.location.trim().is_empty(),
                        egui::Button::new(
                            RichText::new("Scan location    ↗")
                                .family(egui::FontFamily::Name("bold".into()))
                                .color(BG),
                        )
                        .fill(PURPLE)
                        .min_size(Vec2::new(145., 42.)),
                    )
                    .clicked()
                {
                    self.start_scan();
                }
            });

            ui.add_space(3.);

            egui::CollapsingHeader::new(RichText::new("Exclude folders").size(10.).color(MUTED))
                .show(ui, |ui| {
                    muted(
                        ui,
                        "One full folder path per line. Subfolders are excluded too.",
                    );

                    ui.add_enabled(
                        !scanning,
                        egui::TextEdit::multiline(&mut self.excludes)
                            .desired_width(f32::INFINITY)
                            .desired_rows(2)
                            .font(egui::TextStyle::Monospace),
                    );

                    ui.horizontal(|ui| {
                        muted(ui, "Scan workers");

                        ui.add_enabled(
                            !scanning,
                            egui::DragValue::new(&mut self.workers).range(1..=16),
                        );

                        muted(ui, "HDD / network: 1–2 · SSD: 4–8");
                    });
                });
        });

        if let Some(t) = &self.scan {
            ui.add_space(14.);

            card().fill(Color32::from_rgb(33, 30, 45)).show(ui, |ui| {
                ui.set_width(ui.available_width());

                ui.label(
                    RichText::new(if t.cancel.load(Ordering::Relaxed) {
                        "Stopping scan…"
                    } else {
                        "Scanning your files…"
                    })
                    .family(egui::FontFamily::Name("bold".into())),
                );

                muted(
                    ui,
                    format!(
                        "{} files · {} · {:.1}s",
                        count(t.progress.files.load(Ordering::Relaxed)),
                        size_label(t.progress.bytes.load(Ordering::Relaxed)),
                        t.started.elapsed().as_secs_f64()
                    ),
                );

                ui.add(
                    egui::ProgressBar::new(0.5)
                        .animate(true)
                        .fill(PURPLE)
                        .desired_height(4.),
                );

                muted(
                    ui,
                    "Previous results remain available until this scan finishes.",
                );
            });
        }
    }

    fn metrics(&self, ui: &mut egui::Ui) {
        let s = self.snapshot.as_ref();

        let values = [
            (
                "SCANNED SIZE",
                s.map(|s| size_label(s.nodes[0].bytes))
                    .unwrap_or("—".into()),
                "Logical file size".into(),
            ),
            (
                "FILES",
                s.map(|s| count(s.files)).unwrap_or("—".into()),
                s.map(|s| format!("{} folders, including root", count(s.folders)))
                    .unwrap_or("No scan yet".into()),
            ),
            (
                "SCAN TIME",
                s.map(|s| format!("{:.1}s", s.seconds))
                    .unwrap_or("—".into()),
                self.completed.clone(),
            ),
            (
                "SKIPPED LOCATIONS",
                s.map(|s| count(s.errors + s.skipped)).unwrap_or("—".into()),
                s.map(|s| format!("{} errors · {} links / special", s.errors, s.skipped))
                    .unwrap_or("No scan yet".into()),
            ),
        ];

        let cols = if ui.available_width() < 700. { 2 } else { 4 };

        for row in values.chunks(cols) {
            ui.columns(cols, |uis| {
                for (ui, (title, value, hint)) in uis.iter_mut().zip(row) {
                    card().corner_radius(12).inner_margin(16).show(ui, |ui| {
                        ui.set_width(ui.available_width());

                        ui.set_min_height(80.);

                        eyebrow(ui, title);

                        ui.add_space(4.);

                        ui.label(
                            RichText::new(value)
                                .size(27.)
                                .family(egui::FontFamily::Name("bold".into())),
                        );

                        muted(ui, hint);
                    });
                }
            });

            if cols == 2 {
                ui.add_space(6.);
            }
        }
    }

    fn donut(&mut self, ui: &mut egui::Ui, s: &Snapshot) {
        if self.types || self.issues {
            return;
        }

        let View::Folder(folder) = self.view else {
            return;
        };

        if self.busy {
            return;
        }

        let total = s.nodes[folder as usize].bytes;

        let entries: Vec<_> = self
            .result
            .chart_ids
            .iter()
            .map(|&id| (id, s.nodes[id as usize].bytes))
            .collect();

        let slices = chart::slices(&entries, total, 7);

        let mut clicked = None;

        card()
            .fill(Color32::from_rgb(30, 30, 43))
            .inner_margin(24)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                let w = ui.available_width();

                let narrow = w < 650.;

                let name = if folder == 0 {
                    s.root.display().to_string()
                } else {
                    s.nodes[folder as usize].name.to_string_lossy().into_owned()
                };

                let heading = |ui: &mut egui::Ui| {
                    eyebrow(ui, "SPACE BREAKDOWN");

                    ui.add_space(6.);

                    ui.add(
                        egui::Label::new(
                            RichText::new(&name)
                                .size(22.)
                                .family(egui::FontFamily::Name("bold".into())),
                        )
                        .truncate(),
                    )
                    .on_hover_text(&name);

                    muted(ui, "Click a folder to explore inside.");
                };

                if narrow {
                    heading(ui);
                }

                ui.horizontal(|ui| {
                    if !narrow {
                        ui.allocate_ui_with_layout(
                            Vec2::new((w - 270.) / 2.3, 220.),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                ui.set_width((w - 270.) / 2.3);

                                ui.add_space(60.);

                                heading(ui);
                            },
                        );
                    }

                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::splat(220.), egui::Sense::click());

                    let center = rect.center();

                    let p = ui.painter_at(rect);

                    p.circle_filled(center, 96.5, Color32::from_rgb(48, 52, 66));

                    p.circle_filled(center, 71.5, Color32::from_rgb(30, 30, 43));

                    let mut angle = -std::f32::consts::FRAC_PI_2;

                    for (index, slice) in slices.iter().enumerate() {
                        let span = if total == 0 {
                            0.
                        } else {
                            (slice.bytes as f64 / total as f64 * std::f64::consts::TAU) as f32
                        };

                        let gap = 0.022_f32.min(span / 2.);

                        let steps = (span * 40.).ceil().max(1.) as usize;

                        let color = COLORS[if slice.id.is_none() { 7 } else { index }];

                        let mut mesh = egui::Mesh::default();

                        for step in 0..=steps {
                            let a = angle + gap / 2. + (span - gap) * step as f32 / steps as f32;

                            let v = Vec2::angled(a);

                            mesh.colored_vertex(center + v * 71.5, color);

                            mesh.colored_vertex(center + v * 96.5, color);

                            if step > 0 {
                                let i = (step * 2) as u32;

                                mesh.add_triangle(i - 2, i - 1, i);

                                mesh.add_triangle(i - 1, i + 1, i);
                            }
                        }

                        p.add(egui::Shape::mesh(mesh));

                        if let Some(pos) = response.hover_pos() {
                            let v = pos - center;

                            let a = (v.angle() + std::f32::consts::FRAC_PI_2)
                                .rem_euclid(std::f32::consts::TAU);

                            let start = angle + std::f32::consts::FRAC_PI_2;

                            if (71.5..=96.5).contains(&v.length()) && a >= start && a < start + span
                            {
                                let name = slice
                                    .id
                                    .map(|id| {
                                        s.nodes[id as usize].name.to_string_lossy().into_owned()
                                    })
                                    .unwrap_or("Other items".into());

                                response
                                    .clone()
                                    .on_hover_text(format!("{name} · {}", size_label(slice.bytes)));

                                if response.clicked() {
                                    clicked = slice.id;
                                }
                            }
                        }

                        angle += span;
                    }

                    p.text(
                        center - Vec2::new(0., 8.),
                        Align2::CENTER_CENTER,
                        size_label(total),
                        FontId::proportional(24.),
                        TEXT,
                    );

                    p.text(
                        center + Vec2::new(0., 19.),
                        Align2::CENTER_CENTER,
                        "in this folder",
                        FontId::proportional(10.),
                        MUTED,
                    );

                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), 220.),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            ui.add_space(((220. - slices.len() as f32 * 29.) / 2.).max(0.));

                            for (index, slice) in slices.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    let (r, _) = ui
                                        .allocate_exact_size(Vec2::splat(8.), egui::Sense::hover());

                                    ui.painter().rect_filled(
                                        r,
                                        2,
                                        COLORS[if slice.id.is_none() { 7 } else { index }],
                                    );

                                    let name = slice
                                        .id
                                        .map(|id| {
                                            s.nodes[id as usize].name.to_string_lossy().into_owned()
                                        })
                                        .unwrap_or("Other items".into());

                                    let width = (ui.available_width() - 80.).max(25.);

                                    if ui
                                        .add_sized(
                                            [width, 21.],
                                            egui::Button::new(&name)
                                                .frame(false)
                                                .truncate()
                                                .right_text(""),
                                        )
                                        .on_hover_text(name)
                                        .clicked()
                                    {
                                        clicked = slice.id;
                                    }

                                    muted(ui, size_label(slice.bytes));
                                });
                            }
                        },
                    );
                });
            });

        if let Some(id) = clicked {
            self.open(id, ui, s);
        }

        ui.add_space(14.);
    }

    fn export(&mut self, s: &Snapshot) {
        let mut csv = String::from("Name,Size bytes,Files,Path\r\n");

        if self.types {
            for t in s.types.iter().skip(self.page * PAGE).take(PAGE) {
                csv.push_str(&format!(
                    "{},{},{},\r\n",
                    csv_field(&t.extension),
                    t.bytes,
                    t.files
                ));
            }
        } else if self.issues {
            csv = "Path,Reason,Kind\r\n".into();

            for issue in s.issues.iter().skip(self.page * PAGE).take(PAGE) {
                csv.push_str(&format!(
                    "{},{},{}",
                    csv_field(&issue.path.to_string_lossy()),
                    csv_field(&issue.reason),
                    if issue.skipped { "Skipped" } else { "Error" }
                ));

                csv.push_str("\r\n");
            }
        } else {
            for &id in &self.result.ids {
                let n = &s.nodes[id as usize];

                csv.push_str(&format!(
                    "{},{},{},{}\r\n",
                    csv_field(&n.name.to_string_lossy()),
                    n.bytes,
                    n.files,
                    csv_field(&s.path(id).to_string_lossy())
                ));
            }
        }

        let (tx, rx) = mpsc::channel();

        self.export_rx = Some(rx);

        std::thread::spawn(move || {
            let result = if let Some(path) = rfd::FileDialog::new()
                .set_file_name("diskpie-view.csv")
                .add_filter("CSV", &["csv"])
                .save_file()
            {
                std::fs::write(path, csv).map_err(|e| format!("Export failed: {e}"))
            } else {
                Err("Export cancelled".into())
            };

            let _ = tx.send(result);
        });
    }

    fn results(&mut self, ui: &mut egui::Ui) {
        let snapshot = self.snapshot.clone();
        card().inner_margin(0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::Frame::new().inner_margin(22).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        let title = if self.types { "File types" } else if self.issues { "Skipped locations" } else {
                            match self.view { View::Folder(_) => "Folder explorer", View::Largest => "Largest files", View::Search(_) => "Search results", View::Extension(_) => "Files by type" }
                        };
                        ui.label(RichText::new(title).size(17.).family(egui::FontFamily::Name("bold".into())));
                        muted(ui, if self.issues { "Up to 128 read errors and 128 skipped links / special entries listed." } else { "Explore your storage, one folder at a time." });
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_enabled(snapshot.is_some() && !self.busy && self.export_rx.is_none(), egui::Button::new("↓ Export this page")).clicked() {
                            self.export(snapshot.as_ref().unwrap());
                        }
                    });
                });
                ui.add_space(12.);
                ui.horizontal_wrapped(|ui| {
                    if let Some(s) = &snapshot {
                        let mut ancestors=Vec::new(); let mut id=self.folder;
                        while id!=NONE { ancestors.push(id); id=s.nodes[id as usize].parent; }
                        for id in ancestors.into_iter().rev() {
                            let name=if id==0 {"Scan root".into()} else {s.nodes[id as usize].name.to_string_lossy().into_owned()};
                            if ui.link(name).clicked() {self.search.clear();self.navigate(View::Folder(id));}
                            ui.colored_label(MUTED,"/");
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_enabled(snapshot.is_some() && !self.types && !self.issues,egui::TextEdit::singleline(&mut self.search).desired_width(230.).hint_text("⌕ Search all files & folders…").margin(Vec2::new(10.,8.))).changed() { self.search_changed=Some(Instant::now()); }
                    });
                });
            });
            let Some(s)=&snapshot else {
                ui.vertical_centered(|ui| {ui.add_space(48.);ui.label(RichText::new("◔ A clearer picture of your disk").size(20.));muted(ui,"Choose a drive on the left or enter a folder above to start scanning.");ui.add_space(50.);});
                return;
            };
            let widths = { let w=(ui.available_width()-100.).max(300.); [w*0.48,w*0.15,w*0.12,w*0.16,w*0.09] };
            egui::Frame::new().fill(Color32::from_rgb(21,24,32)).inner_margin(egui::Margin::symmetric(24,8)).show(ui,|ui| {
                ui.horizontal(|ui| {
                    for (i,(label,sort)) in [("Name",Some(Sort::Name)),("Size",Some(Sort::Size)),("Files",Some(Sort::Files)),("Share",None),("Actions",None)].into_iter().enumerate() {
                        let label=if sort==Some(self.sort) && !self.types && !self.issues {format!("{label} {}",if self.descending {"↓"} else {"↑"})} else {label.into()};
                        let numeric=i==1||i==2;
                        let response=ui.allocate_ui_with_layout(Vec2::new(widths[i],22.),if numeric{egui::Layout::right_to_left(egui::Align::Center)}else{egui::Layout::left_to_right(egui::Align::Center)},|ui|{ui.set_min_size(Vec2::new(widths[i],22.));ui.add(egui::Button::new(RichText::new(label).size(10.).color(MUTED)).frame(false))}).inner;
                        if response.clicked() && !self.types && !self.issues && let Some(sort)=sort {
                            if self.sort==sort {self.descending = !self.descending;} else {self.sort=sort;self.descending=sort!=Sort::Name;}
                            self.page=0;self.request();
                        }
                    }
                });
            });
            let total=if self.types {s.types.len()} else if self.issues {s.issues.len()} else {self.result.total};
            let length=if self.types || self.issues {total.saturating_sub(self.page*PAGE).min(PAGE)} else {self.result.ids.len()};
            let base=match self.view {View::Folder(id) if !self.types && !self.issues => s.nodes[id as usize].bytes,_=>s.nodes[0].bytes};
            let mut open=None;let mut extension=None;
            if self.busy && !self.types && !self.issues {ui.horizontal(|ui| {ui.spinner();muted(ui,"Loading results…");});}
            else if length==0 {ui.add_space(20.);muted(ui,"No items to display.");ui.add_space(20.);}
            egui::ScrollArea::vertical().id_salt(("results",self.page,self.types,self.issues)).max_height(450.).auto_shrink([false,true]).show_rows(ui,48.,length,|ui,range| {
                for index in range {
                    let row_index=self.page*PAGE+index;
                    let (id,name,bytes,files,detail)=if self.types {
                        let t=&s.types[row_index];(None,if t.extension.is_empty(){"(no extension)".into()}else{format!(".{}",t.extension)},t.bytes,t.files,String::new())
                    } else if self.issues {(None,s.issues[row_index].path.display().to_string(),0,0,s.issues[row_index].reason.clone())} else {
                        let id=self.result.ids[index];let n=&s.nodes[id as usize];
                        (Some(id),n.name.to_string_lossy().into_owned(),n.bytes,n.files,if matches!(self.view,View::Folder(_)){String::new()}else{s.path(id).display().to_string()})
                    };
                    ui.push_id(row_index,|ui| {
                        let row=egui::Frame::new().inner_margin(egui::Margin::symmetric(24,5)).show(ui,|ui| {
                            ui.horizontal(|ui| {
                                ui.allocate_ui_with_layout(Vec2::new(widths[0],38.),egui::Layout::left_to_right(egui::Align::Center),|ui| {
                                    ui.set_min_size(Vec2::new(widths[0],38.));
                                    let directory=id.is_some_and(|id|s.nodes[id as usize].is_dir);
                                    ui.colored_label(if directory{PURPLE}else{MUTED},if directory{"▱"}else{"▤"});
                                    ui.vertical(|ui| {
                                        if ui.add(egui::Label::new(RichText::new(&name).size(12.)).truncate().sense(egui::Sense::click())).on_hover_text(&name).clicked() {
                                            if self.types {extension=Some(s.types[row_index].extension.clone());} else if let Some(id)=id {open=Some(id);} else {ui.ctx().copy_text(name.clone());}
                                        }
                                        if !detail.is_empty() {ui.add(egui::Label::new(RichText::new(&detail).size(10.).color(MUTED)).truncate());}
                                    });
                                });
                                for (width,text) in [(widths[1],if self.issues{"—".into()}else{size_label(bytes)}),(widths[2],if self.issues{"—".into()}else{count(files)})] {
                                    ui.allocate_ui_with_layout(Vec2::new(width,38.),egui::Layout::right_to_left(egui::Align::Center),|ui| {ui.set_min_size(Vec2::new(width,38.));ui.label(text);});
                                }
                                ui.allocate_ui_with_layout(Vec2::new(widths[3],38.),egui::Layout::top_down(egui::Align::LEFT),|ui| {
                                    ui.set_min_size(Vec2::new(widths[3],38.));
                                    if !self.issues {let f=if base==0{0.}else{bytes as f32/base as f32};ui.add_space(5.);ui.add(egui::ProgressBar::new(f).desired_width(widths[3].min(70.)).desired_height(4.).fill(PURPLE));muted(ui,format!("{:.1}%",100.*f));}
                                });
                                if self.issues {
                                    let path=&s.issues[row_index].path;
                                    if ui.small_button("⧉").on_hover_text("Copy path").clicked(){ui.ctx().copy_text(path.display().to_string());}
                                    if ui.add_enabled(!self.demo,egui::Button::new("↗").small()).on_hover_text("Show parent folder").clicked(){let parent=path.parent().unwrap_or(path).to_path_buf();std::thread::spawn(move||reveal(parent));}
                                }
                                if let Some(id)=id {
                                    if ui.small_button("⧉").on_hover_text("Copy path").clicked() {ui.ctx().copy_text(s.path(id).display().to_string());}
                                    if ui.add_enabled(!self.demo,egui::Button::new("↗").small()).on_hover_text("Show in file manager").clicked() {let path=s.path(id);std::thread::spawn(move||reveal(path));}
                                }
                            });
                        }).response.rect;
                        ui.painter().hline(row.x_range(),row.bottom(),Stroke::new(1.0_f32,Color32::from_rgb(39,44,55)));
                    });
                }
            });
            egui::Frame::new().inner_margin(egui::Margin::symmetric(24,14)).show(ui,|ui| {
                ui.horizontal(|ui| {
                    muted(ui,format!("{}–{} of {} items",if total==0{0}else{self.page*PAGE+1},(self.page*PAGE+length).min(total),count(total as u64)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {
                        if ui.add_enabled(!self.busy&&(self.page+1)*PAGE<total,egui::Button::new("Next →")).clicked() {self.page+=1;if !self.types&&!self.issues {self.request();}}
                        if ui.add_enabled(!self.busy&&self.page>0,egui::Button::new("← Previous")).clicked() {self.page-=1;if !self.types&&!self.issues {self.request();}}
                    });
                });
            });
            if let Some(id)=open {self.open(id,ui,s);}
            if let Some(ext)=extension {self.navigate(View::Extension(ext));}
        });
    }
}
impl eframe::App for DiskPieApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll(ctx);
        let wide = ctx.content_rect().width() > 1150.;
        egui::SidePanel::left("navigation")
            .resizable(false)
            .exact_width(if wide { 230. } else { 200. })
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(21, 23, 30))
                    .inner_margin(egui::Margin::symmetric(if wide { 20 } else { 12 }, 22))
                    .stroke(Stroke::new(1.0_f32, LINE)),
            )
            .show(ctx, |ui| self.sidebar(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(if wide {
 40 }
 else {
 25 }
, 30)),
            )
            .show(ctx, |ui| {

                egui::ScrollArea::vertical().id_salt("main").show(ui, |ui| {

                    ui.set_width(ui.available_width());

                    ui.horizontal(|ui| {

                        ui.vertical(|ui| {

                            eyebrow(ui, "KNOW YOUR STORAGE");

                            ui.add_space(3.);

                            ui.label(
                                RichText::new("Storage overview")
                                    .size(30.)
                                    .family(egui::FontFamily::Name("bold".into())),
                            );

                            ui.add_space(2.);

                            muted(
                                ui,
                                self.snapshot
                                    .as_ref()
                                    .map(|s| s.root.display().to_string())
                                    .unwrap_or(
                                        "Find what takes up space. Start with a drive or folder."
                                            .into(),
                                    ),
                            );

                        }
);

                        if wide {

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {

                                    let (r, _) = ui.allocate_exact_size(
                                        Vec2::new(100., 30.),
                                        egui::Sense::hover(),
                                    );

                                    ui.painter()
                                        .rect_filled(r, 15, Color32::from_rgb(27, 43, 40));

                                    ui.painter().text(
                                        r.center(),
                                        Align2::CENTER_CENTER,
                                        "● Local scanner",
                                        FontId::proportional(10.),
                                        MINT,
                                    );

                                }
,
                            );

                        }

                    }
);

                    ui.add_space(22.);

                    self.scan_box(ui);


                    if !self.message.is_empty() {

                        ui.add_space(12.);

                        egui::Frame::new()
                            .fill(Color32::from_rgb(43, 37, 30))
                            .corner_radius(8)
                            .inner_margin(10)
                            .show(ui, |ui| {

                                ui.set_width(ui.available_width());

                                ui.label(
                                    RichText::new(&self.message)
                                        .size(12.)
                                        .color(Color32::from_rgb(241, 191, 114)),
                                );

                            }
);

                    }

                    ui.add_space(20.);

                    self.metrics(ui);

                    ui.add_space(20.);

                    if let Some(s) = self.snapshot.clone() {

                        self.donut(ui, &s);

                    }

                    self.results(ui);

                    ui.add_space(16.);

                    muted(
                        ui,
                        "Sizes are logical file sizes. Links are not followed; permissions and changing files can affect totals.",
                    );

                }
);

            }
);
    }
}
