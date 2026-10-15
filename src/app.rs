//! The main window: folder hierarchy, treemap and file kind statistics, with
//! a synchronized selection — modelled after Disk Inventory X.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bytesize::ByteSize;
use eframe::egui::{
    self, Align, Color32, ColorImage, Key, Layout as ELayout, RichText, ScrollArea, Sense, Stroke,
    TextureHandle, TextureOptions, Vec2,
};

use crate::scan::{self, Progress, Tree, NO_KIND};
use crate::treemap::{self, Layout};
use crate::volumes::{self, Volume};

const MAX_LIST: usize = 1000;

fn fmt_size(bytes: u64) -> String {
    ByteSize(bytes).to_string()
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

enum Screen {
    Start,
    Scanning { path: PathBuf, progress: Arc<Progress>, rx: Receiver<Option<Tree>>, started: Instant },
    Main,
}

struct TreemapCache {
    texture: TextureHandle,
    layout: Layout,
    size: [usize; 2],
    root: usize,
    generation: u64,
}

struct ListCache {
    key: (Option<u32>, String, u64),
    items: Vec<usize>,
    total: usize,
}

pub struct App {
    screen: Screen,
    tree: Option<Tree>,
    /// Bumped whenever the tree changes, to invalidate caches.
    generation: u64,
    zoom: usize,
    selected: Option<usize>,
    hovered: Option<usize>,
    expanded: HashSet<usize>,
    scroll_to_selection: bool,
    treemap: Option<TreemapCache>,
    kind_filter: Option<u32>,
    search: String,
    list: Option<ListCache>,
    show_list: bool,
    show_inspector: bool,
    confirm_trash: Option<usize>,
    error: Option<String>,
    volumes: Vec<Volume>,
    volumes_refreshed: Instant,
}

impl App {
    pub fn new(initial: Option<PathBuf>) -> Self {
        let mut app = Self {
            screen: Screen::Start,
            tree: None,
            generation: 0,
            zoom: 0,
            selected: None,
            hovered: None,
            expanded: HashSet::new(),
            scroll_to_selection: false,
            treemap: None,
            kind_filter: None,
            search: String::new(),
            list: None,
            show_list: false,
            show_inspector: true,
            confirm_trash: None,
            error: None,
            volumes: volumes::list(),
            volumes_refreshed: Instant::now(),
        };
        if let Some(p) = initial {
            app.start_scan(p);
        }
        app
    }

    fn start_scan(&mut self, path: PathBuf) {
        let progress = Arc::new(Progress::default());
        let (tx, rx) = mpsc::channel();
        let (p, prog) = (path.clone(), progress.clone());
        std::thread::spawn(move || {
            let _ = tx.send(scan::scan(&p, &prog));
        });
        self.screen = Screen::Scanning { path, progress, rx, started: Instant::now() };
    }

    fn set_tree(&mut self, tree: Tree) {
        self.tree = Some(tree);
        self.generation += 1;
        self.zoom = 0;
        self.selected = None;
        self.expanded = HashSet::from([0]);
        self.kind_filter = None;
        self.treemap = None;
        self.list = None;
        self.screen = Screen::Main;
    }

    fn select(&mut self, idx: usize) {
        let Some(tree) = &self.tree else { return };
        self.selected = Some(idx);
        let mut p = tree.nodes[idx].parent;
        while let Some(i) = p {
            self.expanded.insert(i);
            p = tree.nodes[i].parent;
        }
        if !tree.is_ancestor(self.zoom, idx) {
            self.zoom = 0;
        }
        self.scroll_to_selection = true;
    }

    fn zoom_in(&mut self) {
        let (Some(tree), Some(sel)) = (&self.tree, self.selected) else { return };
        // Zoom to the child of the current zoom root on the way to the selection.
        let mut target = sel;
        while let Some(p) = tree.nodes[target].parent {
            if p == self.zoom {
                break;
            }
            target = p;
        }
        if !tree.nodes[target].is_dir {
            target = tree.nodes[target].parent.unwrap_or(0);
        }
        self.zoom = target;
    }

    fn zoom_out(&mut self) {
        if let Some(tree) = &self.tree
            && let Some(p) = tree.nodes[self.zoom].parent {
                self.zoom = p;
            }
    }

    fn reveal(&self, idx: usize) {
        if let Some(tree) = &self.tree {
            let _ = std::process::Command::new("open").arg("-R").arg(tree.path(idx)).spawn();
        }
    }

    fn open(&self, idx: usize) {
        if let Some(tree) = &self.tree {
            let _ = std::process::Command::new("open").arg(tree.path(idx)).spawn();
        }
    }

    fn trash(&mut self, idx: usize) {
        let Some(tree) = &mut self.tree else { return };
        if let Err(e) = trash::delete(tree.path(idx)) {
            self.error = Some(format!("Could not move to Trash: {e}"));
            return;
        }
        let parent = tree.nodes[idx].parent.unwrap_or(0);
        if tree.is_ancestor(idx, self.zoom) {
            self.zoom = parent;
        }
        tree.remove(idx);
        self.selected = Some(parent);
        self.generation += 1;
    }

    fn apply(&mut self, action: MenuAction, idx: usize) {
        match action {
            MenuAction::Reveal => self.reveal(idx),
            MenuAction::Open => self.open(idx),
            MenuAction::ZoomIn => self.zoom_in(),
            MenuAction::Trash => self.confirm_trash = Some(idx),
        }
    }

    // ---------------------------------------------------------------- screens

    fn start_screen(&mut self, ctx: &egui::Context) {
        if self.volumes_refreshed.elapsed() > Duration::from_secs(2) {
            self.volumes = volumes::list();
            self.volumes_refreshed = Instant::now();
        }
        ctx.request_repaint_after(Duration::from_secs(2));

        let mut chosen = None;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(30.0);
                ui.heading(RichText::new("Disk Inventory NG").size(28.0));
                ui.label("Choose a volume or folder to see where your disk space has gone.");
                ui.add_space(20.0);
            });
            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - 560.0).max(0.0) / 2.0);
                ui.vertical(|ui| {
                    ui.set_width(560.0);
                    for v in &self.volumes {
                        let used = v.total.saturating_sub(v.free);
                        ui.group(|ui| {
                            ui.set_width(540.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("🖴").size(28.0));
                                ui.vertical(|ui| {
                                    ui.strong(&v.name);
                                    ui.label(format!(
                                        "{} total, {} free — {}",
                                        fmt_size(v.total),
                                        fmt_size(v.free),
                                        v.path.display()
                                    ));
                                    let frac = if v.total > 0 { used as f32 / v.total as f32 } else { 0.0 };
                                    ui.add(egui::ProgressBar::new(frac).desired_width(380.0));
                                });
                                ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                                    if ui.button("Open Volume").clicked() {
                                        chosen = Some(v.path.clone());
                                    }
                                });
                            });
                        });
                    }
                    ui.add_space(12.0);
                    if ui.button("📂 Open Folder…").clicked() {
                        chosen = rfd::FileDialog::new().pick_folder();
                    }
                });
            });
        });
        if let Some(p) = chosen {
            self.start_scan(p);
        }
    }

    fn scanning_screen(&mut self, ctx: &egui::Context) {
        let Screen::Scanning { path, progress, rx, started } = &self.screen else { return };
        match rx.try_recv() {
            Ok(Some(tree)) => return self.set_tree(tree),
            Ok(None) | Err(mpsc::TryRecvError::Disconnected) => {
                self.screen = if self.tree.is_some() { Screen::Main } else { Screen::Start };
                return;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        ctx.request_repaint_after(Duration::from_millis(100));
        let mut cancel = false;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.heading(format!("Scanning {}", path.display()));
                ui.add_space(10.0);
                ui.spinner();
                ui.label(format!(
                    "{} files, {} — {:.0} s",
                    progress.files.load(Ordering::Relaxed),
                    fmt_size(progress.bytes.load(Ordering::Relaxed)),
                    started.elapsed().as_secs_f32()
                ));
                let current = progress.current.lock().map(|s| s.clone()).unwrap_or_default();
                ui.add(egui::Label::new(RichText::new(current).weak().small()).truncate());
                ui.add_space(10.0);
                cancel = ui.button("Cancel").clicked();
            });
        });
        if cancel {
            progress.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn main_screen(&mut self, ctx: &egui::Context) {
        self.shortcuts(ctx);
        self.toolbar(ctx);
        self.status_bar(ctx);
        if self.show_list {
            self.selection_list(ctx);
        }
        egui::SidePanel::left("hierarchy")
            .resizable(true)
            .default_width(340.0)
            .show(ctx, |ui| self.hierarchy(ui));
        egui::SidePanel::right("kinds")
            .resizable(true)
            .default_width(280.0)
            .show(ctx, |ui| self.kinds_panel(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| self.treemap_view(ui));
        self.dialogs(ctx);
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        let (zin, zout, up, reveal) = ctx.input(|i| {
            (
                i.modifiers.command && i.key_pressed(Key::ArrowDown),
                i.modifiers.command && i.key_pressed(Key::ArrowUp),
                !i.modifiers.command && i.key_pressed(Key::ArrowUp) && i.modifiers.alt,
                i.modifiers.command && i.key_pressed(Key::R),
            )
        });
        if zin {
            self.zoom_in();
        }
        if zout {
            self.zoom_out();
        }
        if up
            && let (Some(t), Some(s)) = (&self.tree, self.selected)
                && let Some(p) = t.nodes[s].parent {
                    self.select(p);
                }
        if reveal
            && let Some(s) = self.selected {
                self.reveal(s);
            }
    }

    fn toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let has_sel = self.selected.is_some();
                if ui.button("⏏ Volumes").clicked() {
                    self.screen = Screen::Start;
                }
                if ui.button("⟳ Rescan").clicked()
                    && let Some(t) = &self.tree {
                        self.start_scan(t.path(0));
                    }
                ui.separator();
                if ui.add_enabled(has_sel, egui::Button::new("🔍 Zoom In")).on_hover_text("⌘↓").clicked() {
                    self.zoom_in();
                }
                if ui.add_enabled(self.zoom != 0, egui::Button::new("Zoom Out")).on_hover_text("⌘↑").clicked() {
                    self.zoom_out();
                }
                ui.separator();
                if ui.add_enabled(has_sel, egui::Button::new("Reveal in Finder")).on_hover_text("⌘R").clicked() {
                    self.reveal(self.selected.unwrap());
                }
                if ui.add_enabled(has_sel, egui::Button::new("🗑 Move to Trash")).clicked() {
                    self.confirm_trash = self.selected;
                }
                ui.separator();
                ui.toggle_value(&mut self.show_list, "Selection List");
                ui.toggle_value(&mut self.show_inspector, "Inspector");
            });
        });
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        let Some(tree) = &self.tree else { return };
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let root = &tree.nodes[0];
                ui.label(format!("{} — {} in {} files", tree.path(0).display(), fmt_size(root.size), root.files));
                if let Some(h) = self.hovered.or(self.selected) {
                    ui.separator();
                    ui.add(
                        egui::Label::new(format!("{}  ({})", tree.path(h).display(), fmt_size(tree.nodes[h].size)))
                            .truncate(),
                    );
                }
            });
        });
    }

    fn hierarchy(&mut self, ui: &mut egui::Ui) {
        let Some(tree) = &self.tree else { return };

        // Flatten the expanded part of the tree into rows.
        let mut rows: Vec<(usize, usize)> = Vec::new();
        let mut stack = vec![(0usize, 0usize)];
        while let Some((idx, depth)) = stack.pop() {
            rows.push((idx, depth));
            if self.expanded.contains(&idx) {
                for &c in tree.nodes[idx].children.iter().rev() {
                    stack.push((c, depth + 1));
                }
            }
        }

        let row_h = ui.spacing().interact_size.y;
        let mut area = ScrollArea::both().auto_shrink(false);
        if std::mem::take(&mut self.scroll_to_selection)
            && let Some(pos) = rows.iter().position(|&(i, _)| Some(i) == self.selected) {
                let step = row_h + ui.spacing().item_spacing.y;
                let target = pos as f32 * step - ui.available_height() / 2.0;
                area = area.vertical_scroll_offset(target.max(0.0));
            }

        let mut clicked = None;
        let mut toggled = None;
        let mut zoom_to = None;
        let mut action = None;
        area.show_rows(ui, row_h, rows.len(), |ui, range| {
            for &(idx, depth) in &rows[range] {
                let n = &tree.nodes[idx];
                ui.horizontal(|ui| {
                    ui.add_space(depth as f32 * 14.0);
                    if n.is_dir && !n.children.is_empty() {
                        let icon = if self.expanded.contains(&idx) { "⏷" } else { "⏵" };
                        if ui.add(egui::Button::new(icon).frame(false).min_size(Vec2::new(16.0, 0.0))).clicked() {
                            toggled = Some(idx);
                        }
                    } else {
                        ui.add_space(16.0 + ui.spacing().item_spacing.x);
                    }
                    let color = match n.kind {
                        NO_KIND => Color32::from_rgb(110, 150, 220),
                        k => rgb(tree.kinds[k as usize].color),
                    };
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                    ui.painter().rect_filled(r, 2.0, color);
                    let name = if idx == 0 { tree.path(0).display().to_string() } else { n.name.clone() };
                    let resp = ui.selectable_label(self.selected == Some(idx), name);
                    if resp.clicked() {
                        clicked = Some(idx);
                    }
                    if resp.double_clicked() && n.is_dir {
                        zoom_to = Some(idx);
                    }
                    if resp.secondary_clicked() {
                        clicked = Some(idx);
                    }
                    resp.context_menu(|ui| {
                        if let Some(a) = menu_actions(ui) {
                            action = Some((a, idx));
                        }
                    });
                    let parent_size = n.parent.map(|p| tree.nodes[p].size).unwrap_or(n.size).max(1);
                    let pct = n.size as f32 / parent_size as f32;
                    ui.label(RichText::new(fmt_size(n.size)).weak());
                    ui.add(egui::ProgressBar::new(pct).desired_width(50.0).desired_height(8.0));
                });
            }
        });
        if let Some((a, idx)) = action {
            self.select(idx);
            self.apply(a, idx);
        }

        if let Some(t) = toggled
            && !self.expanded.remove(&t) {
                self.expanded.insert(t);
            }
        if let Some(c) = clicked {
            self.selected = Some(c);
            if self.tree.as_ref().is_some_and(|t| !t.is_ancestor(self.zoom, c)) {
                self.zoom = 0;
            }
        }
        if let Some(z) = zoom_to {
            self.zoom = z;
            self.expanded.insert(z);
        }
    }

    fn kinds_panel(&mut self, ui: &mut egui::Ui) {
        let Some(tree) = &self.tree else { return };
        let selected_kind = self.selected.map(|s| tree.nodes[s].kind);
        let inspector_h = if self.show_inspector { 230.0 } else { 0.0 };

        ui.heading("File Kinds");
        let mut pick = None;
        ScrollArea::vertical()
            .max_height((ui.available_height() - inspector_h).max(100.0))
            .auto_shrink(false)
            .show(ui, |ui| {
                egui::Grid::new("kinds_grid").striped(true).num_columns(4).show(ui, |ui| {
                    ui.label("");
                    ui.strong("Kind");
                    ui.strong("Size");
                    ui.strong("Files");
                    ui.end_row();
                    for (k, kind) in tree.kinds.iter().enumerate() {
                        if kind.count == 0 {
                            continue;
                        }
                        let (r, _) = ui.allocate_exact_size(Vec2::new(14.0, 14.0), Sense::hover());
                        ui.painter().rect_filled(r, 2.0, rgb(kind.color));
                        let highlighted = selected_kind == Some(k as u32) || self.kind_filter == Some(k as u32);
                        if ui.selectable_label(highlighted, &kind.name).clicked() {
                            pick = Some(k as u32);
                        }
                        ui.label(fmt_size(kind.size));
                        ui.label(kind.count.to_string());
                        ui.end_row();
                    }
                });
            });
        if let Some(k) = pick {
            self.kind_filter = if self.kind_filter == Some(k) { None } else { Some(k) };
            self.show_list = true;
        }

        if self.show_inspector {
            ui.separator();
            self.inspector(ui);
        }
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        let Some(tree) = &self.tree else { return };
        ui.heading("Inspector");
        let Some(sel) = self.selected else {
            ui.label(RichText::new("Nothing selected").weak());
            return;
        };
        let n = &tree.nodes[sel];
        let total = tree.nodes[0].size.max(1);
        egui::Grid::new("inspector").num_columns(2).show(ui, |ui| {
            ui.label("Name");
            ui.add(egui::Label::new(RichText::new(&n.name).strong()).truncate());
            ui.end_row();
            ui.label("Where");
            let where_ = n.parent.map(|p| tree.path(p).display().to_string()).unwrap_or_default();
            ui.add(egui::Label::new(where_).truncate());
            ui.end_row();
            ui.label("Kind");
            ui.label(tree.kind_name(sel));
            ui.end_row();
            ui.label("Size");
            ui.label(format!("{} ({} bytes)", fmt_size(n.size), n.size));
            ui.end_row();
            ui.label("Share");
            ui.label(format!("{:.2} % of total", n.size as f64 * 100.0 / total as f64));
            ui.end_row();
            if n.is_dir {
                ui.label("Files");
                ui.label(n.files.to_string());
                ui.end_row();
            }
            ui.label("Modified");
            ui.label(fmt_age(n.mtime));
            ui.end_row();
        });
        ui.horizontal(|ui| {
            if ui.button("Reveal in Finder").clicked() {
                self.reveal(sel);
            }
            if ui.button("Open").clicked() {
                self.open(sel);
            }
        });
    }

    fn treemap_view(&mut self, ui: &mut egui::Ui) {
        let Some(tree) = &self.tree else { return };
        let rect = ui.available_rect_before_wrap();
        let ppp = ui.ctx().pixels_per_point();
        let size = [(rect.width() * ppp).max(1.0) as usize, (rect.height() * ppp).max(1.0) as usize];

        let stale = self
            .treemap
            .as_ref()
            .is_none_or(|c| c.size != size || c.root != self.zoom || c.generation != self.generation);
        if stale {
            let (img, layout) = treemap::render(tree, self.zoom, size[0], size[1]);
            let image = ColorImage::from_rgba_unmultiplied(size, &img.rgba);
            match &mut self.treemap {
                Some(c) if c.size == size => {
                    c.texture.set(image, TextureOptions::NEAREST);
                    c.layout = layout;
                }
                _ => {
                    self.treemap = Some(TreemapCache {
                        texture: ui.ctx().load_texture("treemap", image, TextureOptions::NEAREST),
                        layout,
                        size,
                        root: self.zoom,
                        generation: self.generation,
                    })
                }
            }
            let c = self.treemap.as_mut().unwrap();
            c.size = size;
            c.root = self.zoom;
            c.generation = self.generation;
        }
        let cache = self.treemap.as_ref().unwrap();

        let resp = ui.allocate_rect(rect, Sense::click());
        let painter = ui.painter_at(rect);
        painter.image(
            cache.texture.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );

        let to_screen = |r: treemap::Rect| {
            egui::Rect::from_min_max(
                rect.min + Vec2::new(r[0], r[1]) / ppp,
                rect.min + Vec2::new(r[2], r[3]) / ppp,
            )
        };

        // Outline the selection (or its closest visible ancestor).
        if let Some(mut s) = self.selected {
            loop {
                if let Some(&r) = cache.layout.rects.get(&s) {
                    painter.rect_stroke(to_screen(r).shrink(1.0), 0.0, Stroke::new(2.0_f32, Color32::WHITE));
                    painter.rect_stroke(to_screen(r), 0.0, Stroke::new(1.0_f32, Color32::BLACK));
                    break;
                }
                match tree.nodes[s].parent {
                    Some(p) => s = p,
                    None => break,
                }
            }
        }

        self.hovered = resp.hover_pos().and_then(|p| {
            let v = (p - rect.min) * ppp;
            cache.layout.hit(v.x, v.y)
        });
        if let Some(h) = self.hovered {
            resp.clone().on_hover_ui_at_pointer(|ui| {
                ui.strong(&tree.nodes[h].name);
                ui.label(format!("{} — {}", fmt_size(tree.nodes[h].size), tree.kind_name(h)));
            });
        }

        let hovered = self.hovered;
        if (resp.clicked() || resp.secondary_clicked())
            && let Some(h) = hovered {
                self.select(h);
            }
        if resp.double_clicked() && hovered.is_some() {
            self.zoom_in();
        }
        let mut action = None;
        resp.context_menu(|ui| action = menu_actions(ui));
        if let (Some(a), Some(sel)) = (action, self.selected) {
            self.apply(a, sel);
        }
    }

    fn selection_list(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("selection_list")
            .resizable(true)
            .default_height(220.0)
            .show(ctx, |ui| {
                let Some(tree) = &self.tree else { return };
                ui.horizontal(|ui| {
                    ui.heading("Selection List");
                    ui.label("Kind:");
                    let label = self.kind_filter.map_or("All kinds".to_owned(), |k| tree.kinds[k as usize].name.clone());
                    egui::ComboBox::from_id_salt("kind_filter").selected_text(label).show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.kind_filter, None, "All kinds");
                        for (k, kind) in tree.kinds.iter().enumerate().take(200) {
                            ui.selectable_value(&mut self.kind_filter, Some(k as u32), &kind.name);
                        }
                    });
                    ui.label("Search:");
                    ui.text_edit_singleline(&mut self.search);
                });

                let key = (self.kind_filter, self.search.to_lowercase(), self.generation);
                if self.list.as_ref().is_none_or(|l| l.key != key) {
                    let mut all: Vec<usize> = Vec::new();
                    let mut stack = vec![0usize];
                    while let Some(i) = stack.pop() {
                        let n = &tree.nodes[i];
                        stack.extend_from_slice(&n.children);
                        if n.is_dir {
                            continue;
                        }
                        if key.0.is_some_and(|k| k != n.kind) {
                            continue;
                        }
                        if !key.1.is_empty() && !n.name.to_lowercase().contains(&key.1) {
                            continue;
                        }
                        all.push(i);
                    }
                    all.sort_by(|&a, &b| tree.nodes[b].size.cmp(&tree.nodes[a].size));
                    let total = all.len();
                    all.truncate(MAX_LIST);
                    self.list = Some(ListCache { key, items: all, total });
                }
                let list = self.list.as_ref().unwrap();
                ui.label(
                    RichText::new(format!("{} files (showing the largest {})", list.total, list.items.len().min(MAX_LIST)))
                        .weak(),
                );

                let mut clicked = None;
                let row_h = ui.spacing().interact_size.y;
                ScrollArea::vertical().auto_shrink(false).show_rows(ui, row_h, list.items.len(), |ui, range| {
                    for &i in &list.items[range] {
                        ui.horizontal(|ui| {
                            let n = &tree.nodes[i];
                            ui.label(RichText::new(fmt_size(n.size)).monospace());
                            if ui.selectable_label(self.selected == Some(i), &n.name).clicked() {
                                clicked = Some(i);
                            }
                            ui.label(RichText::new(tree.path(n.parent.unwrap_or(0)).display().to_string()).weak());
                        });
                    }
                });
                if let Some(c) = clicked {
                    self.select(c);
                }
            });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(idx) = self.confirm_trash {
            let name = self.tree.as_ref().map(|t| t.path(idx).display().to_string()).unwrap_or_default();
            let mut decision = None;
            egui::Window::new("Move to Trash?")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(format!("Move \"{name}\" to the Trash?"));
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            decision = Some(false);
                        }
                        if ui.button("Move to Trash").clicked() {
                            decision = Some(true);
                        }
                    });
                });
            match decision {
                Some(true) => {
                    self.confirm_trash = None;
                    self.trash(idx);
                }
                Some(false) => self.confirm_trash = None,
                None => {}
            }
        }
        if let Some(err) = self.error.clone() {
            egui::Window::new("Error").collapsible(false).show(ctx, |ui| {
                ui.label(err);
                if ui.button("OK").clicked() {
                    self.error = None;
                }
            });
        }
    }
}

#[derive(Clone, Copy)]
enum MenuAction {
    Reveal,
    Open,
    ZoomIn,
    Trash,
}

/// Context menu shared by the hierarchy and the treemap.
fn menu_actions(ui: &mut egui::Ui) -> Option<MenuAction> {
    let mut action = None;
    for (label, a) in [
        ("Reveal in Finder", MenuAction::Reveal),
        ("Open", MenuAction::Open),
        ("Zoom In", MenuAction::ZoomIn),
        ("Move to Trash…", MenuAction::Trash),
    ] {
        if ui.button(label).clicked() {
            action = Some(a);
            ui.close_menu();
        }
    }
    action
}

fn fmt_age(mtime: i64) -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let secs = (now - mtime).max(0);
    match secs {
        s if s < 3600 => format!("{} minutes ago", s / 60),
        s if s < 86400 => format!("{} hours ago", s / 3600),
        s if s < 86400 * 365 => format!("{} days ago", s / 86400),
        s => format!("{:.1} years ago", s as f64 / (86400.0 * 365.25)),
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        match self.screen {
            Screen::Start => self.start_screen(ctx),
            Screen::Scanning { .. } => self.scanning_screen(ctx),
            Screen::Main => self.main_screen(ctx),
        }
    }
}
