use diskpie::{
    chart,
    model::{NONE, NodeId, Snapshot, size_label},
    query::{QueryResult, View},
    tasks::{QueryTask, ScanTask, publish},
};
use eframe::egui::{self, Align2, Color32, FontId, RichText, Stroke, Vec2};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering, mpsc},
    time::Duration,
};

const ACCENT: Color32 = Color32::from_rgb(86, 218, 181);
const COLORS: [Color32; 8] = [
    ACCENT,
    Color32::from_rgb(101, 161, 245),
    Color32::from_rgb(179, 139, 250),
    Color32::from_rgb(247, 186, 103),
    Color32::from_rgb(238, 126, 151),
    Color32::from_rgb(94, 196, 221),
    Color32::from_rgb(175, 204, 116),
    Color32::from_rgb(170, 177, 194),
];

pub struct DiskPieApp {
    location: String,
    workers: usize,
    scan: Option<ScanTask>,
    snapshot: Option<Arc<Snapshot>>,
    query: QueryTask,
    result: QueryResult,
    view: View,
    folder: NodeId,
    search: String,
    busy: bool,
    types: bool,
    issues: bool,
    message: String,
    picker: Option<mpsc::Receiver<Option<PathBuf>>>,
    demo: bool,
}

impl DiskPieApp {
    pub fn new(cc: &eframe::CreationContext<'_>, root: Option<PathBuf>, demo: bool) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = Color32::from_rgb(19, 23, 32);
        style.visuals.window_fill = Color32::from_rgb(25, 30, 41);
        style.visuals.extreme_bg_color = Color32::from_rgb(13, 17, 24);
        style.visuals.selection.bg_fill = Color32::from_rgb(35, 79, 73);
        style.visuals.selection.stroke.color = ACCENT;
        style.spacing.item_spacing = Vec2::new(10.0, 10.0);
        style.spacing.button_padding = Vec2::new(12.0, 7.0);
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(15.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(15.0));
        cc.egui_ctx.set_style(style);
        let mut app = Self {
            location: root
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            workers: 4,
            scan: None,
            snapshot: None,
            query: QueryTask::default(),
            result: QueryResult::default(),
            view: View::Folder(0),
            folder: 0,
            search: String::new(),
            busy: false,
            types: false,
            issues: false,
            message: "Choose a location. Nothing is scanned or saved automatically.".into(),
            picker: None,
            demo,
        };
        if demo {
            app.snapshot = Some(Arc::new(demo_snapshot()));
            app.message = "Demo · synthetic data only".into();
            app.navigate(View::Folder(0));
        } else if root.is_some() {
            app.start_scan();
        }
        app
    }

    fn start_scan(&mut self) {
        if self.scan.is_some() || self.location.trim().is_empty() {
            return;
        }
        self.demo = false;
        self.scan = Some(ScanTask::start(
            PathBuf::from(self.location.trim()),
            self.workers,
        ));
        self.message = "Scanning metadata. Previous results remain available.".into();
    }

    fn navigate(&mut self, view: View) {
        if let Some(snapshot) = &self.snapshot {
            if let View::Folder(id) = view {
                self.folder = id;
            }
            self.view = view.clone();
            self.types = false;
            self.issues = false;
            self.result = QueryResult::default();
            self.busy = true;
            self.query.request(snapshot.clone(), view);
        }
    }

    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(task) = &self.scan {
            match task.receiver.try_recv() {
                Ok(result) => {
                    match publish(&mut self.snapshot, result, &task.cancel) {
                        Ok(()) => {
                            let s = self.snapshot.as_ref().unwrap();
                            self.message = format!(
                                "Scan finished in {:.2}s · {} read errors · {} links / special entries skipped",
                                s.seconds, s.errors, s.skipped
                            );
                            self.search.clear();
                            self.navigate(View::Folder(0));
                        }
                        Err(message) => self.message = message,
                    }
                    self.scan = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.scan = None;
                    self.message = "Scan worker stopped; previous result kept".into();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        while let Ok((generation, result)) = self.query.receiver.try_recv() {
            if generation == self.query.current {
                self.result = result;
                self.busy = false;
            }
        }
        if let Some(picker) = &self.picker {
            match picker.try_recv() {
                Ok(path) => {
                    if let Some(path) = path {
                        self.location = path.display().to_string();
                    }
                    self.picker = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.picker = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.scan.is_some() || self.busy || self.picker.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("DISKPIE").size(22.0).strong().color(ACCENT));
            ui.separator();
            ui.label(RichText::new("Native storage explorer").color(Color32::GRAY));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new("LOCAL ONLY  ·  NO UPLOADS")
                        .small()
                        .color(ACCENT),
                );
            });
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let scanning = self.scan.is_some();
            let response = ui.add_enabled(
                !scanning,
                egui::TextEdit::singleline(&mut self.location)
                    .hint_text("Drive or folder path…")
                    .desired_width((ui.available_width() - 350.0).max(100.0)),
            );
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.start_scan();
            }
            if ui
                .add_enabled(
                    !scanning && self.picker.is_none(),
                    egui::Button::new("Browse…"),
                )
                .clicked()
            {
                let (tx, rx) = mpsc::channel();
                self.picker = Some(rx);
                let ctx = ui.ctx().clone();
                std::thread::spawn(move || {
                    let _ = tx.send(rfd::FileDialog::new().pick_folder());
                    ctx.request_repaint();
                });
            }
            ui.label("Workers");
            ui.add_enabled(
                !scanning,
                egui::DragValue::new(&mut self.workers).range(1..=16),
            );
            if scanning {
                if ui.button("Stop scan").clicked() {
                    self.scan
                        .as_ref()
                        .unwrap()
                        .cancel
                        .store(true, Ordering::Relaxed);
                    self.message = "Stopping after current filesystem calls…".into();
                }
            } else if ui
                .add_enabled(
                    !self.location.trim().is_empty(),
                    egui::Button::new(RichText::new("Scan location").color(ACCENT)),
                )
                .clicked()
            {
                self.start_scan();
            }
        });
        ui.add_space(5.0);
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(12.0);
        ui.label(RichText::new("EXPLORE").small().color(Color32::GRAY));
        if ui
            .selectable_label(
                !self.types && !self.issues && matches!(self.view, View::Folder(_)),
                "Folders",
            )
            .clicked()
        {
            self.search.clear();
            self.navigate(View::Folder(self.folder));
        }
        if ui
            .selectable_label(
                !self.types && !self.issues && self.view == View::Largest,
                "Largest files",
            )
            .clicked()
        {
            self.search.clear();
            self.navigate(View::Largest);
        }
        if ui.selectable_label(self.types, "File types").clicked() {
            self.types = true;
            self.issues = false;
        }
        if ui
            .selectable_label(self.issues, "Skipped / errors")
            .clicked()
        {
            self.issues = true;
            self.types = false;
        }
        ui.add_space(18.0);
        ui.label(
            RichText::new("SEARCH ALL NAMES")
                .small()
                .color(Color32::GRAY),
        );
        if ui
            .add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Literal name…")
                    .desired_width(f32::INFINITY),
            )
            .changed()
        {
            if self.search.is_empty() {
                self.navigate(View::Folder(self.folder));
            } else {
                self.navigate(View::Search(self.search.clone()));
            }
        }
        ui.add_space(20.0);
        ui.separator();
        if let Some(task) = &self.scan {
            ui.heading("Scanning");
            ui.label(format!(
                "{} files",
                task.progress.files.load(Ordering::Relaxed)
            ));
            ui.label(format!(
                "{} folders",
                task.progress.folders.load(Ordering::Relaxed)
            ));
            ui.label(size_label(task.progress.bytes.load(Ordering::Relaxed)));
            ui.label(format!(
                "{:.1}s elapsed",
                task.started.elapsed().as_secs_f64()
            ));
            ui.label(format!(
                "{} errors",
                task.progress.errors.load(Ordering::Relaxed)
            ));
        } else if let Some(s) = &self.snapshot {
            ui.label(
                RichText::new(size_label(s.nodes[0].bytes))
                    .size(30.0)
                    .strong(),
            );
            ui.label("logical bytes in scan");
            ui.add_space(10.0);
            ui.label(format!("{} files", s.files));
            ui.label(format!("{} folders", s.folders));
            ui.label(format!("{:.2}s scan time", s.seconds));
            if s.errors > 0 {
                ui.colored_label(
                    Color32::from_rgb(247, 186, 103),
                    "Partial: some locations unreadable",
                );
            }
        }
        ui.add_space(24.0);
        ui.label(
            RichText::new("Metadata only. No file contents read. Results stay in memory.")
                .small()
                .color(Color32::GRAY),
        );
        ui.label(
            RichText::new("HDD / network: try 1–2 workers. SSD: try 4–8.")
                .small()
                .color(Color32::GRAY),
        );
    }

    fn content(&mut self, ui: &mut egui::Ui) {
        let Some(snapshot) = self.snapshot.clone() else {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.25);
                ui.label(RichText::new("Find your space.").size(38.0).strong());
                ui.add_space(12.0);
                ui.label("Choose a drive or folder above to see what takes up space.");
                ui.add_space(8.0);
                ui.colored_label(
                    ACCENT,
                    "Fast metadata scan · Native interface · Private by default",
                );
                ui.add_space(24.0);
                ui.label("No automatic scan. No saved inventory. No deletion.");
            });
            return;
        };
        if self.issues {
            ui.heading("Coverage and read errors");
            ui.label(format!(
                "{} errors · {} links / reparse points / special files skipped",
                snapshot.errors, snapshot.skipped
            ));
            ui.label("Links are not followed. Errors are sampled (maximum 128); counters include every skip.");
            egui::ScrollArea::vertical().show_rows(ui, 28.0, snapshot.issues.len(), |ui, range| {
                for index in range {
                    ui.add(egui::Label::new(&snapshot.issues[index]).truncate())
                        .on_hover_text(&snapshot.issues[index]);
                }
            });
            return;
        }
        if self.types {
            ui.heading("File types");
            ui.label("All files in this scan. Select an extension for its largest files.");
            let mut extension = None;
            egui::ScrollArea::vertical().show_rows(ui, 34.0, snapshot.types.len(), |ui, range| {
                for index in range {
                    let item = &snapshot.types[index];
                    ui.horizontal(|ui| {
                        let name = if item.extension.is_empty() {
                            "(no extension)".into()
                        } else {
                            format!(".{}", item.extension)
                        };
                        if ui
                            .add_sized([200.0, 28.0], egui::Button::new(name))
                            .clicked()
                        {
                            extension = Some(item.extension.clone());
                        }
                        ui.label(size_label(item.bytes));
                        ui.label(format!("{} files", item.files));
                    });
                }
            });
            if let Some(ext) = extension {
                self.navigate(View::Extension(ext));
            }
            return;
        }
        if let View::Folder(folder) = self.view {
            let mut ancestors = Vec::new();
            let mut id = folder;
            while id != NONE {
                ancestors.push(id);
                id = snapshot.nodes[id as usize].parent;
            }
            ui.horizontal_wrapped(|ui| {
                for id in ancestors.into_iter().rev() {
                    let text = if id == 0 {
                        "Scan root".to_owned()
                    } else {
                        snapshot.nodes[id as usize]
                            .name
                            .to_string_lossy()
                            .into_owned()
                    };
                    if ui.link(text).clicked() {
                        self.navigate(View::Folder(id));
                    }
                    ui.label("/");
                }
            });
            ui.add(egui::Label::new(snapshot.path(folder).display().to_string()).truncate())
                .on_hover_text(snapshot.path(folder).display().to_string());
        } else {
            ui.heading(match &self.view {
                View::Largest => "Largest files".into(),
                View::Search(s) => format!("Search: {s}"),
                View::Extension(e) => format!(
                    "File type: {}",
                    if e.is_empty() { "(no extension)" } else { e }
                ),
                _ => String::new(),
            });
            ui.label("Across the complete scan");
        }
        if self.busy {
            ui.label("Preparing view…");
            return;
        }
        if matches!(self.view, View::Folder(_)) && ui.available_height() > 360.0 {
            self.donut(ui, &snapshot);
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} entries", self.result.total)).strong());
            if self.result.total > self.result.ids.len() {
                ui.label(format!(
                    "Showing largest {}. Use search to find other entries.",
                    self.result.ids.len()
                ));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label("SIZE · LARGEST FIRST");
            });
        });
        ui.separator();
        let mut open = None;
        let total = match self.view {
            View::Folder(id) => snapshot.nodes[id as usize].bytes,
            _ => snapshot.nodes[0].bytes,
        };
        egui::ScrollArea::vertical()
            .id_salt("entries")
            .auto_shrink([false, false])
            .show_rows(ui, 28.0, self.result.ids.len(), |ui, range| {
                for index in range {
                    let id = self.result.ids[index];
                    let node = &snapshot.nodes[id as usize];
                    ui.push_id(id, |ui| {
                        ui.horizontal(|ui| {
                            let available = ui.available_width();
                            ui.colored_label(
                                if node.is_dir { ACCENT } else { Color32::GRAY },
                                if node.is_dir { "DIR" } else { "FILE" },
                            );
                            let response = ui
                                .allocate_ui_with_layout(
                                    Vec2::new((available - 390.0).max(70.0), 28.0),
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        ui.set_min_width((available - 390.0).max(70.0));
                                        ui.add(
                                            egui::Label::new(node.name.to_string_lossy())
                                                .truncate()
                                                .sense(egui::Sense::click()),
                                        )
                                    },
                                )
                                .inner;
                            if response.hovered() {
                                response
                                    .clone()
                                    .on_hover_text(snapshot.path(id).display().to_string());
                            }
                            if response.clicked() && node.is_dir {
                                open = Some(id);
                            }
                            ui.allocate_ui_with_layout(
                                Vec2::new(85.0, 28.0),
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.set_min_width(85.0);
                                    ui.label(size_label(node.bytes));
                                },
                            );
                            let fraction = if total == 0 {
                                0.0
                            } else {
                                node.bytes as f64 / total as f64
                            };
                            ui.add(
                                egui::ProgressBar::new(fraction as f32)
                                    .desired_width(64.0)
                                    .text(format!("{:.1}%", fraction * 100.0)),
                            );
                            if ui
                                .small_button("Copy")
                                .on_hover_text("Copy full path")
                                .clicked()
                            {
                                ui.ctx().copy_text(snapshot.path(id).display().to_string());
                            }
                            if ui
                                .add_enabled(!self.demo, egui::Button::new("Show").small())
                                .on_hover_text("Reveal in file manager")
                                .clicked()
                            {
                                let path = snapshot.path(id);
                                std::thread::spawn(move || reveal(path));
                            }
                        });
                    });
                }
            });
        if let Some(id) = open {
            self.search.clear();
            self.navigate(View::Folder(id));
        }
    }

    fn donut(&mut self, ui: &mut egui::Ui, snapshot: &Snapshot) {
        let View::Folder(folder) = self.view else {
            return;
        };
        let total = snapshot.nodes[folder as usize].bytes;
        let entries: Vec<_> = self
            .result
            .ids
            .iter()
            .take(24)
            .map(|id| (*id, snapshot.nodes[*id as usize].bytes))
            .collect();
        let slices = chart::slices(&entries, total, 24);
        let mut open = None;
        ui.horizontal(|ui| {
            let (rect, response) =
                ui.allocate_exact_size(Vec2::new(250.0, 230.0), egui::Sense::click());
            let center = rect.center();
            let outer = 100.0;
            let inner = 68.0;
            let painter = ui.painter_at(rect);
            painter.circle_stroke(center, outer, Stroke::new(1.0_f32, Color32::from_gray(50)));
            let mut angle = -std::f32::consts::FRAC_PI_2;
            for (index, slice) in slices.iter().enumerate() {
                let span = (slice.bytes as f64 / total as f64 * std::f64::consts::TAU) as f32;
                let steps = (span * 35.0).ceil().max(1.0) as usize;
                let color = if slice.id.is_none() {
                    Color32::from_gray(80)
                } else {
                    COLORS[index % COLORS.len()]
                };
                let mut mesh = egui::Mesh::default();
                for step in 0..=steps {
                    let a = angle + span * step as f32 / steps as f32;
                    let direction = Vec2::angled(a);
                    mesh.colored_vertex(center + direction * inner, color);
                    mesh.colored_vertex(center + direction * outer, color);
                    if step > 0 {
                        let i = (step * 2) as u32;
                        mesh.add_triangle(i - 2, i - 1, i);
                        mesh.add_triangle(i - 1, i + 1, i);
                    }
                }
                painter.add(egui::Shape::mesh(mesh));
                if let Some(pos) = response.hover_pos() {
                    let vector = pos - center;
                    let radius = vector.length();
                    let mouse = (vector.angle() + std::f32::consts::FRAC_PI_2)
                        .rem_euclid(std::f32::consts::TAU);
                    let start = angle + std::f32::consts::FRAC_PI_2;
                    if radius >= inner && radius <= outer && mouse >= start && mouse < start + span
                    {
                        let name = slice
                            .id
                            .map(|id| {
                                snapshot.nodes[id as usize]
                                    .name
                                    .to_string_lossy()
                                    .into_owned()
                            })
                            .unwrap_or("Other entries".into());
                        response
                            .clone()
                            .on_hover_text(format!("{name}\n{}", size_label(slice.bytes)));
                        if response.clicked() {
                            open = slice.id.filter(|id| snapshot.nodes[*id as usize].is_dir);
                        }
                    }
                }
                angle += span;
            }
            painter.text(
                center - Vec2::new(0.0, 8.0),
                Align2::CENTER_CENTER,
                size_label(total),
                FontId::proportional(23.0),
                Color32::WHITE,
            );
            painter.text(
                center + Vec2::new(0.0, 20.0),
                Align2::CENTER_CENTER,
                "IN THIS FOLDER",
                FontId::proportional(10.0),
                Color32::GRAY,
            );
            ui.vertical(|ui| {
                ui.label(RichText::new("Where the space goes").size(20.0).strong());
                ui.label(
                    RichText::new("Select a folder to explore deeper")
                        .small()
                        .color(Color32::GRAY),
                );
                for (index, slice) in slices.iter().take(5).enumerate() {
                    ui.horizontal(|ui| {
                        let (dot, _) =
                            ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                        ui.painter()
                            .circle_filled(dot.center(), 4.0, COLORS[index % COLORS.len()]);
                        let name = slice
                            .id
                            .map(|id| {
                                snapshot.nodes[id as usize]
                                    .name
                                    .to_string_lossy()
                                    .into_owned()
                            })
                            .unwrap_or("Other entries".into());
                        if ui
                            .add(egui::Button::new(name).frame(false).truncate())
                            .clicked()
                        {
                            open = slice.id.filter(|id| snapshot.nodes[*id as usize].is_dir);
                        }
                        ui.label(RichText::new(size_label(slice.bytes)).color(Color32::GRAY));
                    });
                }
            });
        });
        if let Some(id) = open {
            self.navigate(View::Folder(id));
        }
    }
}

impl eframe::App for DiskPieApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll(ctx);
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| self.toolbar(ui));
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(&self.message).small());
            });
        });
        egui::SidePanel::left("navigation")
            .resizable(false)
            .exact_width(220.0)
            .show(ctx, |ui| self.sidebar(ui));
        egui::CentralPanel::default().show(ctx, |ui| self.content(ui));
    }
}

fn reveal(path: PathBuf) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let display = path.to_string_lossy();
        let display = if let Some(unc) = display.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{unc}")
        } else {
            display
                .strip_prefix(r"\\?\")
                .unwrap_or(&display)
                .to_string()
        };
        let _ = std::process::Command::new("explorer.exe")
            .raw_arg(format!("/select,\"{display}\""))
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let parent = if path.is_dir() {
            path.as_path()
        } else {
            path.parent().unwrap_or(&path)
        };
        let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
    }
}

fn demo_snapshot() -> Snapshot {
    use diskpie::model::{FileType, Node};
    let mut s = Snapshot {
        root: PathBuf::from("Demo"),
        files: 240_000,
        folders: 6,
        seconds: 1.24,
        ..Default::default()
    };
    s.nodes.push(Node {
        name: "Demo".into(),
        bytes: 0,
        parent: NONE,
        first_child: 1,
        next_sibling: NONE,
        is_dir: true,
    });
    for (index, (name, gib)) in [
        ("Projects", 42),
        ("Media", 26),
        ("Applications", 18),
        ("Archives", 9),
        ("Documents", 3),
    ]
    .into_iter()
    .enumerate()
    {
        let id = index as u32 + 1;
        let bytes = gib * 1024 * 1024 * 1024;
        s.nodes[0].bytes += bytes;
        s.nodes.push(Node {
            name: name.into(),
            bytes,
            parent: 0,
            first_child: NONE,
            next_sibling: if id == 5 { NONE } else { id + 1 },
            is_dir: true,
        });
    }
    for parent in 1..=5 {
        let bytes = s.nodes[parent].bytes;
        let first = s.nodes.len() as u32;
        s.nodes[parent].first_child = first;
        for i in 0..1000 {
            s.nodes.push(Node {
                name: format!("sample-{i:04}.bin").into(),
                bytes: bytes / 1000 + if i == 0 { bytes % 1000 } else { 0 },
                parent: parent as u32,
                first_child: NONE,
                next_sibling: if i == 999 { NONE } else { first + i + 1 },
                is_dir: false,
            });
        }
    }
    s.files = 5000;
    s.types.push(FileType {
        extension: "bin".into(),
        bytes: s.nodes[0].bytes,
        files: s.files,
    });
    s
}
