#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use eframe::egui;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// ── Lang ──────────────────────────────────────────────────────────────────────

struct Lang {
    // Auth
    title:            &'static str,
    sudo_prompt:      &'static str,
    sudo_placeholder: &'static str,
    unlock:           &'static str,
    enter_password:   &'static str,
    wrong_password:   &'static str,
    // Toolbar / empty state
    add_service:      &'static str,
    add_hint:         &'static str,
    no_services:      &'static str,
    // Table header
    hdr_status:       &'static str,
    hdr_service:      &'static str,
    // Buttons
    btn_stop:         &'static str,
    btn_start:        &'static str,
    btn_disable:      &'static str,
    btn_enable:       &'static str,
    btn_remove:       &'static str,
    // Hover hints
    hint_stop:        &'static str,
    hint_start:       &'static str,
    hint_disable:     &'static str,
    hint_enable:      &'static str,
    hint_remove:      &'static str,
    // Add dialog
    dlg_title:        &'static str,
    dlg_svc_name:     &'static str,
    dlg_command:      &'static str,
    dlg_add:          &'static str,
    dlg_cancel:       &'static str,
}

const RU: Lang = Lang {
    title:            "Сервисный менеджер",
    sudo_prompt:      "Введите пароль sudo для управления сервисами:",
    sudo_placeholder: "пароль sudo…",
    unlock:           "Разблокировать",
    enter_password:   "Введите пароль",
    wrong_password:   "Неверный пароль",
    add_service:      "+ добавить сервис",
    add_hint:         "Создать симлинк /var/service/<name> → /etc/sv/<name>",
    no_services:      "Сервисы не найдены",
    hdr_status:       "STATUS",
    hdr_service:      "SERVICE",
    btn_stop:         "остановить",
    btn_start:        "запустить",
    btn_disable:      "отключить",
    btn_enable:       "включить",
    btn_remove:       "удалить",
    hint_stop:        "sv down — остановить сервис",
    hint_start:       "sv up — запустить сервис",
    hint_disable:     "touch /etc/sv/<name>/down — отключить автозапуск",
    hint_enable:      "rm /etc/sv/<name>/down — включить автозапуск",
    hint_remove:      "rm /var/service/<name> — убрать из автозагрузки",
    dlg_title:        "Добавить сервис",
    dlg_svc_name:     "Имя сервиса:",
    dlg_command:      "Команда:",
    dlg_add:          "Добавить",
    dlg_cancel:       "Отмена",
};

const EN: Lang = Lang {
    title:            "Service Manager",
    sudo_prompt:      "Enter your sudo password to manage services:",
    sudo_placeholder: "sudo password…",
    unlock:           "Unlock",
    enter_password:   "Enter password",
    wrong_password:   "Wrong password",
    add_service:      "+ add service",
    add_hint:         "Create symlink /var/service/<name> → /etc/sv/<name>",
    no_services:      "No services found",
    hdr_status:       "STATUS",
    hdr_service:      "SERVICE",
    btn_stop:         "stop",
    btn_start:        "start",
    btn_disable:      "disable",
    btn_enable:       "enable",
    btn_remove:       "remove",
    hint_stop:        "sv down — stop service",
    hint_start:       "sv up — start service",
    hint_disable:     "touch /etc/sv/<name>/down — disable autostart",
    hint_enable:      "rm /etc/sv/<name>/down — enable autostart",
    hint_remove:      "rm /var/service/<name> — remove from autostart",
    dlg_title:        "Add Service",
    dlg_svc_name:     "Service name:",
    dlg_command:      "Command:",
    dlg_add:          "Add",
    dlg_cancel:       "Cancel",
};

fn detect_lang() -> &'static Lang {
    for var in &["LANG", "LANGUAGE", "LC_ALL", "LC_MESSAGES"] {
        if let Ok(val) = std::env::var(var) {
            if val.to_lowercase().starts_with("ru") {
                return &RU;
            }
        }
    }
    &EN
}

// ── sudo helpers ──────────────────────────────────────────────────────────────

fn sudo(pw: &str, args: &[&str]) -> (bool, String) {
    let mut child = match Command::new("sudo")
        .arg("-S").args(args)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn()
    { Ok(c) => c, Err(e) => return (false, e.to_string()) };
    if let Some(mut s) = child.stdin.take() {
        let _ = s.write_all(format!("{}\n", pw).as_bytes());
    }
    match child.wait_with_output() {
        Ok(o) => (o.status.success(),
                  String::from_utf8_lossy(&o.stdout).to_string()
                + &String::from_utf8_lossy(&o.stderr).to_string()),
        Err(e) => (false, e.to_string()),
    }
}

fn run(args: &[&str]) -> String {
    Command::new(args[0]).args(&args[1..])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}

// ── service data ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct Svc {
    name:    String,
    state:   String, // "run" | "down" | ...
    enabled: bool,   // нет файла /etc/sv/<name>/down
    pid:     String,
    command: String,
    time:    String,
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for nc in chars.by_ref() {
                if nc.is_ascii_alphabetic() { break; }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn load_services(pw: &str) -> Vec<Svc> {
    let names_raw = run(&["ls", "/var/service"]);
    let mut svcs = Vec::new();

    for name in names_raw.lines().map(str::trim).filter(|s| !s.is_empty()) {
        let (_, status_raw) = sudo(pw, &["sv", "status", &format!("/var/service/{}", name)]);
        let status = strip_ansi(&status_raw);
        let status = status.trim();

        let state = if status.starts_with("run") {
            "run".to_string()
        } else if status.starts_with("down") {
            "down".to_string()
        } else {
            status.split(':').next().unwrap_or("?").trim().to_string()
        };

        let pid = {
            let mut p = "-".to_string();
            if let Some(start) = status.find("(pid ") {
                let rest = &status[start + 5..];
                if let Some(end) = rest.find(')') {
                    p = rest[..end].trim().to_string();
                }
            }
            p
        };

        let time = {
            let mut secs_opt: Option<u64> = None;
            for word in status.split_whitespace() {
                let w = word.trim_end_matches(|c: char| !c.is_alphanumeric());
                if w.ends_with('s') {
                    let digits = &w[..w.len()-1];
                    if digits.chars().all(|c| c.is_ascii_digit()) {
                        secs_opt = digits.parse().ok();
                        break;
                    }
                }
            }
            if let Some(secs) = secs_opt {
                if secs < 60 { format!("{}s", secs) }
                else if secs < 3600 {
                    let m = secs / 60; let s = secs % 60;
                    if s == 0 { format!("{}m", m) } else { format!("{}m{}s", m, s) }
                } else if secs < 86400 {
                    let h = secs / 3600; let m = (secs % 3600) / 60;
                    if m == 0 { format!("{}h", h) } else { format!("{}h{}m", h, m) }
                } else {
                    let d = secs / 86400; let h = (secs % 86400) / 3600;
                    if h == 0 { format!("{}d", d) } else { format!("{}d{}h", d, h) }
                }
            } else {
                String::new()
            }
        };

        let command = {
            let run_file = format!("/etc/sv/{}/run", name);
            let out = run(&["cat", &run_file]);
            out.lines()
                .rev()
                .find(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
                .map(|l| {
                    l.split_whitespace()
                        .next()
                        .unwrap_or("")
                        .rsplit('/')
                        .next()
                        .unwrap_or("")
                        .to_string()
                })
                .unwrap_or_default()
        };

        let down_file = format!("/etc/sv/{}/down", name);
        let enabled = !std::path::Path::new(&down_file).exists();

        svcs.push(Svc { name: name.to_string(), state, enabled, pid, command, time });
    }

    svcs.sort_by(|a, b| a.name.cmp(&b.name));
    svcs
}

// ── app ───────────────────────────────────────────────────────────────────────

enum Action {
    Start(String),
    Stop(String),
    Enable(String),
    Disable(String),
    Remove(String),
    Add(String, String), // src, dst (full paths)
}

struct AddDialog {
    svc_name:    String,
    src_line:    String,
    dst_line:    String,
    src_edited:  bool,
    dst_edited:  bool,
    just_opened: bool, // фокус на имя только при первом показе
}

#[derive(PartialEq)]
enum Screen { Auth, Main }

struct App {
    lang:      &'static Lang,
    screen:    Screen,
    svcs:      Arc<Mutex<Vec<Svc>>>,
    loading:   Arc<Mutex<bool>>,
    busy:      Arc<Mutex<std::collections::HashSet<String>>>,
    last_load: Instant,

    password:     String,
    sudo_error:   Option<String>,
    auth_pending: Arc<Mutex<Option<bool>>>, // результат проверки пароля
    add_dlg:      Option<AddDialog>,
}

impl App {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            lang:         detect_lang(),
            screen:       Screen::Auth,
            svcs:         Arc::new(Mutex::new(Vec::new())),
            loading:      Arc::new(Mutex::new(false)),
            busy:         Arc::new(Mutex::new(std::collections::HashSet::new())),
            last_load:    Instant::now() - Duration::from_secs(999),
            password:     String::new(),
            sudo_error:   None,
            auth_pending: Arc::new(Mutex::new(None)),
            add_dlg:      None,
        }
    }

    fn reload(&mut self) {
        {
            let mut l = self.loading.lock().unwrap();
            if *l { return; }
            *l = true;
        }
        self.last_load = Instant::now();
        let svcs    = Arc::clone(&self.svcs);
        let loading = Arc::clone(&self.loading);
        let pw      = self.password.clone();
        thread::spawn(move || {
            *svcs.lock().unwrap()    = load_services(&pw);
            *loading.lock().unwrap() = false;
        });
    }

    fn dispatch(&mut self, action: Action) {
        let pw    = self.password.clone();
        let busy  = Arc::clone(&self.busy);
        let svcs  = Arc::clone(&self.svcs);
        let loading = Arc::clone(&self.loading);

        thread::spawn(move || {
            let name = match &action {
                Action::Start(n)|Action::Stop(n)|Action::Enable(n)
                |Action::Disable(n)|Action::Remove(n) => n.clone(),
                Action::Add(_, dst) => dst.clone(),
            };
            busy.lock().unwrap().insert(name.clone());

            match action {
                Action::Start(n)   => { sudo(&pw, &["sv", "up",   &n]); }
                Action::Stop(n)    => { sudo(&pw, &["sv", "down", &n]); }
                Action::Enable(n)  => {
                    let path = format!("/etc/sv/{}/down", n);
                    sudo(&pw, &["rm", "-f", &path]);
                }
                Action::Disable(n) => {
                    let path = format!("/etc/sv/{}/down", n);
                    sudo(&pw, &["touch", &path]);
                }
                Action::Remove(n)  => {
                    let path = format!("/var/service/{}", n);
                    sudo(&pw, &["rm", "-f", &path]);
                }
                Action::Add(src, dst) => {
                    sudo(&pw, &["ln", "-s", &src, &dst]);
                }
            }

            busy.lock().unwrap().remove(&name);
            // перезагружаем список
            *loading.lock().unwrap() = true;
            *svcs.lock().unwrap() = load_services(&pw);
            *loading.lock().unwrap() = false;
        });
    }

    fn request(&mut self, action: Action) {
        self.dispatch(action);
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(500));
        let l = self.lang;

        // ── Экран авторизации ─────────────────────────────────────────────────
        if self.screen == Screen::Auth {
            // Проверяем результат фонового потока
            let auth_result = *self.auth_pending.lock().unwrap();
            if let Some(ok) = auth_result {
                *self.auth_pending.lock().unwrap() = None;
                if ok {
                    self.screen = Screen::Main;
                    self.sudo_error = None;
                    self.reload();
                } else {
                    self.sudo_error = Some(l.wrong_password.into());
                    self.password.clear();
                }
            }

            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add_space(80.0);
                ui.vertical_centered(|ui| {
                    ui.heading(egui::RichText::new(l.title).size(22.0));
                    ui.add_space(24.0);
                    ui.label(l.sudo_prompt);
                    ui.add_space(8.0);
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.password)
                            .password(true)
                            .hint_text(l.sudo_placeholder)
                            .desired_width(260.0),
                    );
                    resp.request_focus();
                    if let Some(ref err) = self.sudo_error {
                        ui.add_space(6.0);
                        ui.colored_label(egui::Color32::from_rgb(220, 60, 60), err.as_str());
                    }
                    ui.add_space(14.0);
                    let is_checking = self.auth_pending.lock().unwrap().is_none()
                        && self.sudo_error.as_deref() == Some("");
                    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
                    let btn = ui.add_enabled(
                        !is_checking,
                        egui::Button::new(l.unlock).min_size(egui::vec2(100.0, 28.0)),
                    );
                    if btn.clicked() || enter {
                        if self.password.is_empty() {
                            self.sudo_error = Some(l.enter_password.into());
                        } else {
                            self.sudo_error = Some(" ".into());
                            let pw2 = self.password.clone();
                            let res = Arc::clone(&self.auth_pending);
                            thread::spawn(move || {
                                let (ok, _) = sudo(&pw2, &["true"]);
                                *res.lock().unwrap() = Some(ok);
                            });
                        }
                    }
                });
            });
            return;
        }

        // Автообновление каждые 5с
        if self.last_load.elapsed() > Duration::from_secs(5)
            && !*self.loading.lock().unwrap()
        {
            self.reload();
        }

        // ── add service dialog ────────────────────────────────────────────────
        let mut do_add: Option<(String, String)> = None;
        let mut close_add = false;
        if let Some(ref mut dlg) = self.add_dlg {
            let mut svc_name    = dlg.svc_name.clone();
            let mut src_line    = dlg.src_line.clone();
            let mut dst_line    = dlg.dst_line.clone();
            let mut src_edited  = dlg.src_edited;
            let mut dst_edited  = dlg.dst_edited;
            let mut just_opened = dlg.just_opened;
            let mut open = true;
            egui::Window::new(l.dlg_title)
                .collapsible(false).resizable(false)
                .default_width(440.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    // ── поле имени сервиса ────────────────────────────────────
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(l.dlg_svc_name).size(13.0));
                        ui.add_space(4.0);
                        let name_resp = ui.add(
                            egui::TextEdit::singleline(&mut svc_name)
                                .desired_width(180.0)
                                .hint_text("myservice")
                                .font(egui::TextStyle::Monospace),
                        );
                        if just_opened { name_resp.request_focus(); just_opened = false; }
                    });

                    // автодублирование имени в строки если они не редактировались вручную
                    if svc_name != dlg.svc_name {
                        if !src_edited {
                            let base = src_line.rfind('/').map(|i| &src_line[..=i]).unwrap_or("/etc/sv/");
                            src_line = format!("{}{}", base, svc_name);
                        }
                        if !dst_edited {
                            let base = dst_line.rfind('/').map(|i| &dst_line[..=i]).unwrap_or("/var/service/");
                            dst_line = format!("{}{}", base, svc_name);
                        }
                    }

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(6.0);

                    ui.label(egui::RichText::new(l.dlg_command).size(12.0).color(egui::Color32::GRAY));
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("ln -s").monospace().size(13.0)
                            .color(egui::Color32::from_rgb(180,180,180)));
                        ui.add_space(4.0);
                        let src_resp = ui.add(
                            egui::TextEdit::singleline(&mut src_line)
                                .desired_width(260.0)
                                .font(egui::TextStyle::Monospace),
                        );
                        if src_resp.changed() { src_edited = true; }
                        ui.label(egui::RichText::new("\\").monospace().size(13.0)
                            .color(egui::Color32::from_rgb(180,180,180)));
                    });

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("      ").monospace().size(13.0));
                        ui.add_space(4.0);
                        let dst_resp = ui.add(
                            egui::TextEdit::singleline(&mut dst_line)
                                .desired_width(260.0)
                                .font(egui::TextStyle::Monospace),
                        );
                        if dst_resp.changed() { dst_edited = true; }
                    });

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                        let ok_btn = ui.add_enabled(
                            !svc_name.trim().is_empty(),
                            egui::Button::new(l.dlg_add),
                        );
                        if (ok_btn.clicked() || enter) && !svc_name.trim().is_empty() {
                            do_add = Some((src_line.clone(), dst_line.clone()));
                        }
                        if ui.button(l.dlg_cancel).clicked() {
                            close_add = true;
                        }
                    });
                });

            dlg.svc_name    = svc_name;
            dlg.src_line    = src_line;
            dlg.dst_line    = dst_line;
            dlg.src_edited  = src_edited;
            dlg.dst_edited  = dst_edited;
            dlg.just_opened = just_opened;
            if !open { close_add = true; }
        }
        if close_add { self.add_dlg = None; }

        if let Some((src, dst)) = do_add {
            self.add_dlg = None;
            self.request(Action::Add(src, dst));
        }

        // ── toolbar ───────────────────────────────────────────────────────────
        egui::TopBottomPanel::bottom("bottom").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                let add_btn = egui::Button::new(
                    egui::RichText::new(l.add_service).size(13.0)
                        .color(egui::Color32::from_rgb(80,210,80))
                )
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(80,210,80)));

                if ui.add(add_btn).on_hover_text(l.add_hint)
                    .clicked()
                {
                    self.add_dlg = Some(AddDialog {
                        svc_name:    String::new(),
                        src_line:    "/etc/sv/".into(),
                        dst_line:    "/var/service/".into(),
                        src_edited:  false,
                        dst_edited:  false,
                        just_opened: true,
                    });
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    if *self.loading.lock().unwrap() {
                        ui.spinner();
                    }
                });
            });
            ui.add_space(4.0);
        });

        // ── main list ─────────────────────────────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            let svcs = self.svcs.lock().unwrap().clone();
            let busy = self.busy.lock().unwrap().clone();

            if svcs.is_empty() {
                ui.centered_and_justified(|ui| {
                    if *self.loading.lock().unwrap() { ui.spinner(); }
                    else {
                        ui.label(egui::RichText::new(l.no_services).color(egui::Color32::GRAY));
                    }
                });
                return;
            }

            let gray = egui::Color32::GRAY;

            // ── фиксированные ширины правой части (прибита к правому краю) ───────
            let gap        =  8.0_f32;
            let w_del      =  60.0_f32;
            let w_time     =  52.0_f32;
            let w_cmd      =  90.0_f32;
            let w_pid      =  52.0_f32;
            // ширины кнопок — подогнаны под текст без лишних полей
            let w_btn_stop = 110.0_f32;
            let w_btn_en   = 100.0_f32;
            let icon_w     =  24.0_f32;
            // суммарная ширина правой части (PID+COMMAND+TIME+удалить + отступы)
            let right_fixed = gap + w_pid + gap + w_cmd + gap + w_time + gap + w_del + gap;

            // helper: рисует строку заголовка
            let draw_row = |ui: &mut egui::Ui, row_rect: egui::Rect, is_header: bool,
                            _label: &str, pid: &str, cmd: &str, time_s: &str| {
                let p   = ui.painter();
                let cy  = row_rect.center().y;
                let r   = row_rect.right();
                let fhdr = egui::FontId::monospace(11.0);
                let fval = egui::FontId::monospace(13.0);
                let (f, c_pid, c_cmd, c_time) = if is_header {
                    (fhdr.clone(), gray, gray, gray)
                } else {
                    (fval.clone(),
                     egui::Color32::from_rgb(180,120,220),
                     egui::Color32::from_rgb(160,160,200),
                     gray)
                };
                // удалить — правый край
                let x_del_l = r - gap - w_del;
                // TIME
                let x_time_r = x_del_l - gap;
                let x_time_l = x_time_r - w_time;
                // COMMAND
                let x_cmd_r = x_time_l - gap;
                let x_cmd_l = x_cmd_r - w_cmd;
                // PID
                let x_pid_r = x_cmd_l - gap;

                p.text(egui::pos2(x_pid_r,  cy), egui::Align2::RIGHT_CENTER, pid,    f.clone(), c_pid);
                p.text(egui::pos2(x_cmd_r,  cy), egui::Align2::RIGHT_CENTER, cmd,    f.clone(), c_cmd);
                p.text(egui::pos2(x_time_r, cy), egui::Align2::RIGHT_CENTER, time_s, f,         c_time);
                x_del_l
            };

            // ── шапка ────────────────────────────────────────────────────────────
            let (hdr_rect, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), 20.0), egui::Sense::hover());
            draw_row(ui, hdr_rect, true, l.hdr_service, "PID", "COMMAND", "TIME");
            // STATUS над иконкой
            {
                let p = ui.painter();
                let cy = hdr_rect.center().y;
                p.text(egui::pos2(hdr_rect.left() + 2.0, cy), egui::Align2::LEFT_CENTER,
                    l.hdr_status, egui::FontId::monospace(11.0), gray);
            }
            // Рисуем "SERVICE" через child_ui
            {
                let left_end2 = hdr_rect.right() - right_fixed;
                let left_rect = egui::Rect::from_min_max(
                    egui::pos2(hdr_rect.left() + icon_w + 4.0, hdr_rect.top()),
                    egui::pos2(left_end2, hdr_rect.bottom()));
                let mut lui = ui.child_ui(left_rect, egui::Layout::left_to_right(egui::Align::Center));
                let btns_w = w_btn_stop + gap + w_btn_en + gap * 2.0;
                let name_w = (left_rect.width() - btns_w).max(60.0);
                lui.add_sized([name_w, 20.0],
                    egui::Label::new(egui::RichText::new(l.hdr_service).monospace().size(11.0).color(gray)));
            }
            ui.separator();

            // ── список ───────────────────────────────────────────────────────────
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                for svc in &svcs {
                    let is_busy   = busy.contains(&svc.name);
                    let is_run    = svc.state == "run";
                    let no_dialog = self.add_dlg.is_none();
                    let row_h     = 28.0_f32;
                    let avail_w   = ui.available_width();

                    let (row_rect, _) = ui.allocate_exact_size(
                        egui::vec2(avail_w, row_h), egui::Sense::hover());

                    // ── иконка ✓ / ✗ ─────────────────────────────────────────────
                    {
                        let p  = ui.painter();
                        let cx = row_rect.left() + icon_w / 2.0;
                        let cy = row_rect.center().y;
                        let color = if is_run { egui::Color32::from_rgb(80,210,80) }
                                    else      { egui::Color32::from_rgb(200,70,70) };
                        let s = egui::Stroke::new(2.0, color);
                        if is_run {
                            p.line_segment([egui::pos2(cx-5.0, cy+1.0), egui::pos2(cx-1.0, cy+5.0)], s);
                            p.line_segment([egui::pos2(cx-1.0, cy+5.0), egui::pos2(cx+6.0, cy-4.0)], s);
                        } else {
                            p.line_segment([egui::pos2(cx-4.0, cy-4.0), egui::pos2(cx+4.0, cy+4.0)], s);
                            p.line_segment([egui::pos2(cx+4.0, cy-4.0), egui::pos2(cx-4.0, cy+4.0)], s);
                        }
                    }

                    // ── правая часть: PID COMMAND TIME (текст через painter) ──────
                    let pid_str  = if svc.pid != "-" && !svc.pid.is_empty() { svc.pid.as_str() } else { "" };
                    let x_del_l  = draw_row(ui, row_rect, false, "", pid_str, &svc.command, &svc.time);

                    // ── кнопка удалить ────────────────────────────────────────────
                    {
                        let by = row_rect.center().y - 13.0;
                        let br = egui::Rect::from_min_size(
                            egui::pos2(x_del_l, by), egui::vec2(w_del, 26.0));
                        let mut del_ui = ui.child_ui(br, egui::Layout::left_to_right(egui::Align::Center));
                        let del_btn = del_ui.add_sized([w_del, 26.0], egui::Button::new(
                            egui::RichText::new(l.btn_remove).size(13.0)
                                .color(egui::Color32::from_rgb(220,80,80))
                        ).stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(220,80,80))))
                        .on_hover_text(l.hint_remove);
                        if del_btn.clicked() && no_dialog {
                            self.request(Action::Remove(svc.name.clone()));
                        }
                    }

                    // ── левая часть: имя + кнопки ─────────────────────────────────
                    let left_end2 = row_rect.right() - right_fixed;
                    let left_rect = egui::Rect::from_min_max(
                        egui::pos2(row_rect.left() + icon_w + 4.0, row_rect.top()),
                        egui::pos2(left_end2, row_rect.bottom()));
                    let mut lui = ui.child_ui(left_rect, egui::Layout::left_to_right(egui::Align::Center));

                    let btns_w = w_btn_stop + gap + w_btn_en + gap * 2.0;
                    let name_w = (left_rect.width() - btns_w).max(60.0);
                    lui.add_sized([name_w, row_h],
                        egui::Label::new(egui::RichText::new(&svc.name).monospace().size(13.0)));

                    if is_busy {
                        lui.spinner();
                    } else {
                        let (stop_label, stop_color, stop_hint) = if is_run {
                            (l.btn_stop,  egui::Color32::from_rgb(220,80,80),  l.hint_stop)
                        } else {
                            (l.btn_start, egui::Color32::from_rgb(80,210,80),  l.hint_start)
                        };
                        let sb = lui.add_sized([w_btn_stop, 26.0], egui::Button::new(
                            egui::RichText::new(stop_label).size(13.0).color(stop_color)
                        ).stroke(egui::Stroke::new(1.0, stop_color))).on_hover_text(stop_hint);
                        if sb.clicked() && no_dialog {
                            self.request(if is_run { Action::Stop(svc.name.clone()) }
                                         else      { Action::Start(svc.name.clone()) });
                        }

                        lui.add_space(gap);

                        let (en_label, en_color, en_hint) = if svc.enabled {
                            (l.btn_disable, egui::Color32::from_rgb(200,160,40), l.hint_disable)
                        } else {
                            (l.btn_enable,  egui::Color32::from_rgb(100,180,255), l.hint_enable)
                        };
                        let eb = lui.add_sized([w_btn_en, 26.0], egui::Button::new(
                            egui::RichText::new(en_label).size(13.0).color(en_color)
                        ).stroke(egui::Stroke::new(1.0, en_color))).on_hover_text(en_hint);
                        if eb.clicked() && no_dialog {
                            self.request(if svc.enabled { Action::Disable(svc.name.clone()) }
                                         else           { Action::Enable(svc.name.clone()) });
                        }
                    }

                    ui.separator();
                }
            });
        });
    }
}

fn main() -> eframe::Result<()> {
    let title = detect_lang().title;
    eframe::run_native(
        title,
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title(title)
                .with_inner_size([700.0, 440.0])
                .with_min_inner_size([480.0, 280.0]),
            ..Default::default()
        },
        Box::new(|cc| Box::new(App::new(cc))),
    )
}
