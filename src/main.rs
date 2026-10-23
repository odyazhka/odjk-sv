#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use eframe::egui::{self, Align2, Color32, CursorIcon, FontId, RichText, Sense, Stroke};
use std::collections::HashSet;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// ── цвета ─────────────────────────────────────────────────────────────────────

const GREEN:   Color32 = Color32::from_rgb(80, 210, 80);
const KINOVAR: Color32 = Color32::from_rgb(227, 66, 52);  // киноварь
const PUNCH:   Color32 = Color32::from_rgb(224, 17, 95);  // пунцовый
const BLUE:    Color32 = Color32::from_rgb(100, 180, 255);
const RED_X:   Color32 = Color32::from_rgb(200, 70, 70);
const PID_COL: Color32 = Color32::from_rgb(240, 0, 255);
const CMD_COL: Color32 = Color32::from_rgb(160, 160, 200);
const BTN_BG:  Color32 = Color32::from_gray(22);
const YELLOW:  Color32 = Color32::from_rgb(255, 200, 0);  // избранное (залито)
const STAR_OFF: Color32 = Color32::from_gray(120);        // избранное (обводка)

const NAME_W: f32 = 120.0;
const BTN_W:  f32 = 100.0;
const PID_W:  f32 = 40.0;
const TIME_W: f32 = 50.0;
const CMD_W:  f32 = 100.0;

// ── Lang ──────────────────────────────────────────────────────────────────────

struct Lang {
    title:            &'static str,
    sudo_prompt:      &'static str,
    sudo_placeholder: &'static str,
    unlock:           &'static str,
    enter_password:   &'static str,
    wrong_password:   &'static str,
    no_services:      &'static str,
    hdr_status:       &'static str,
    hdr_service:      &'static str,
    hdr_pid:          &'static str,
    hdr_time:         &'static str,
    hdr_command:      &'static str,
    active_hdr:       &'static str,
    inactive_hdr:     &'static str,
    btn_stop:         &'static str,
    btn_start:        &'static str,
    btn_remove:       &'static str,
    btn_disable:      &'static str,
    btn_enable:       &'static str,
    hint_stop:        &'static str,
    hint_start:       &'static str,
    hint_remove:      &'static str,
    hint_enable:      &'static str,
    hint_delete:      &'static str,
    add_service:      &'static str,
    add_hint:         &'static str,
    hint_fav:         &'static str,
    dlg_title:        &'static str,
    dlg_name:         &'static str,
    dlg_add:          &'static str,
    dlg_cancel:       &'static str,
    dlg_bad_name:     &'static str,
    dlg_exists:       &'static str,
    confirm_title:    &'static str,
    confirm_unlink:   &'static str,
    confirm_delete:   &'static str,
}

const RU: Lang = Lang {
    title:            "Сервисный менеджер",
    sudo_prompt:      "Введите пароль sudo для управления сервисами:",
    sudo_placeholder: "пароль sudo…",
    unlock:           "Разблокировать",
    enter_password:   "Введите пароль",
    wrong_password:   "Неверный пароль",
    no_services:      "Сервисы не найдены",
    hdr_status:       "STATUS",
    hdr_service:      "SERVICE",
    hdr_pid:          "PID",
    hdr_time:         "TIME",
    hdr_command:      "COMMAND",
    active_hdr:       "включённые сервисы (/var/service/)",
    inactive_hdr:     "выключенные сервисы (/etc/sv/)",
    btn_stop:         "остановить",
    btn_start:        "запустить",
    btn_remove:       "удалить",
    btn_disable:      "отключить",
    btn_enable:       "включить",
    hint_stop:        "sv down — остановить сервис",
    hint_start:       "sv up — запустить сервис",
    hint_remove:      "rm /var/service/<n> — убрать из /var/service (остаётся в /etc/sv)",
    hint_enable:      "ln -s /etc/sv/<n> /var/service/<n> — включить",
    hint_delete:      "rm -rf /etc/sv/<n> — удалить сервис полностью",
    add_service:      "+ добавить сервис",
    add_hint:         "Создать /etc/sv/<n>/run и открыть папку",
    hint_fav:         "Избранное: наверх списка",
    dlg_title:        "Добавить сервис",
    dlg_name:         "Имя сервиса:",
    dlg_add:          "Добавить",
    dlg_cancel:       "Отмена",
    dlg_bad_name:     "Допустимо: a-z, 0-9, точка, _ и -",
    dlg_exists:       "Такой сервис уже есть в /etc/sv",
    confirm_title:    "Подтверждение",
    confirm_unlink:   "Отключить работающий сервис?",
    confirm_delete:   "Удалить навсегда из /etc/sv:",
};

const EN: Lang = Lang {
    title:            "Service Manager",
    sudo_prompt:      "Enter your sudo password to manage services:",
    sudo_placeholder: "sudo password…",
    unlock:           "Unlock",
    enter_password:   "Enter password",
    wrong_password:   "Wrong password",
    no_services:      "No services found",
    hdr_status:       "STATUS",
    hdr_service:      "SERVICE",
    hdr_pid:          "PID",
    hdr_time:         "TIME",
    hdr_command:      "COMMAND",
    active_hdr:       "enabled services (/var/service/)",
    inactive_hdr:     "disabled services (/etc/sv/)",
    btn_stop:         "stop",
    btn_start:        "start",
    btn_remove:       "remove",
    btn_disable:      "disable",
    btn_enable:       "enable",
    hint_stop:        "sv down — stop service",
    hint_start:       "sv up — start service",
    hint_remove:      "rm /var/service/<n> — remove from /var/service (stays in /etc/sv)",
    hint_enable:      "ln -s /etc/sv/<n> /var/service/<n> — enable",
    hint_delete:      "rm -rf /etc/sv/<n> — delete service completely",
    add_service:      "+ add service",
    add_hint:         "Create /etc/sv/<n>/run and open the folder",
    hint_fav:         "Favorite: pin to top",
    dlg_title:        "Add Service",
    dlg_name:         "Service name:",
    dlg_add:          "Add",
    dlg_cancel:       "Cancel",
    dlg_bad_name:     "Allowed: a-z, 0-9, dot, _ and -",
    dlg_exists:       "This service already exists in /etc/sv",
    confirm_title:    "Confirm",
    confirm_unlink:   "Disable running service?",
    confirm_delete:   "Delete permanently from /etc/sv:",
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

// ── sudo / shell helpers ──────────────────────────────────────────────────────

fn sudo(pw: &str, args: &[&str]) -> (bool, String) {
    let mut child = match Command::new("sudo")
        .arg("-S").args(args)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return (false, e.to_string()),
    };
    if let Some(mut s) = child.stdin.take() {
        let _ = s.write_all(format!("{}\n", pw).as_bytes());
    }
    match child.wait_with_output() {
        Ok(o) => (
            o.status.success(),
            String::from_utf8_lossy(&o.stdout).to_string()
                + &String::from_utf8_lossy(&o.stderr),
        ),
        Err(e) => (false, e.to_string()),
    }
}

fn sudo_ok(pw: &str, args: &[&str]) -> Result<String, String> {
    let (ok, out) = sudo(pw, args);
    if ok { Ok(out) } else { Err(out.trim().to_string()) }
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
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

fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('.')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

// ── service data ──────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
struct Svc {
    name:    String,
    path:    String,   // путь, по которому вызывается sv
    running: bool,
    pid:     String,
    time:    String,
    command: String,      // укороченная версия для колонки
    command_full: String, // полная командная строка для подсказки
    fav:     bool,
}

#[derive(Clone, Default)]
struct Data {
    active:   Vec<Svc>, // /var/service
    inactive: Vec<Svc>, // /etc/sv без того, что уже в /var/service
}

fn fmt_secs(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        let (m, s) = (secs / 60, secs % 60);
        if s == 0 { format!("{}m", m) } else { format!("{}m{}s", m, s) }
    } else if secs < 86400 {
        let (h, m) = (secs / 3600, (secs % 3600) / 60);
        if m == 0 { format!("{}h", h) } else { format!("{}h{}m", h, m) }
    } else {
        let (d, h) = (secs / 86400, (secs % 86400) / 3600);
        if h == 0 { format!("{}d", d) } else { format!("{}d{}h", d, h) }
    }
}

fn parse_status(status: &str) -> (bool, String, String) {
    let running = status.starts_with("run");
    let pid = status
        .find("(pid ")
        .and_then(|s| {
            let rest = &status[s + 5..];
            rest.find(')').map(|e| rest[..e].trim().to_string())
        })
        .unwrap_or_default();
    let time = status
        .split_whitespace()
        .find_map(|w| {
            let w = w.trim_end_matches(|c: char| !c.is_alphanumeric());
            let d = w.strip_suffix('s')?;
            if !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()) {
                d.parse::<u64>().ok()
            } else {
                None
            }
        })
        .map(fmt_secs)
        .unwrap_or_default();
    (running, pid, time)
}

/// Имя процесса для COMMAND, как в vsv: только arg0 из /proc/<pid>/cmdline.
/// Для остановленных сервисов — пусто.
fn cmd_from_pid(pid: &str) -> String {
    std::fs::read(format!("/proc/{}/cmdline", pid))
        .map(|raw| {
            raw.split(|b| *b == 0)
                .next()
                .map(|p| String::from_utf8_lossy(p).into_owned())
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

fn list_dir(dir: &str) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| !n.starts_with('.'))
                .collect()
        })
        .unwrap_or_default()
}

fn make_svc(pw: &str, dir: &str, name: &str) -> Svc {
    let path = format!("{}/{}", dir, name);
    let (_, raw) = sudo(pw, &["sv", "status", &path]);
    let (running, pid, time) = parse_status(strip_ansi(&raw).trim());
    let command_full = if running { cmd_from_pid(&pid) } else { String::new() };
    let mut command = command_full.clone();
    // больше 17 символов: первые 14 и «...», итого 17
    if command.chars().count() > 17 {
        command = command.chars().take(14).collect::<String>() + "...";
    }
    Svc {
        name: name.to_string(),
        path,
        running,
        pid,
        time,
        command,
        command_full,
        fav: false, // проставляется в load()
    }
}

fn fav_file() -> std::path::PathBuf {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_default()
        .join(".local/sv")
}

fn load_favs() -> HashSet<String> {
    std::fs::read_to_string(fav_file())
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

fn save_favs(favs: &HashSet<String>) -> Result<(), String> {
    let path = fav_file();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut names: Vec<&String> = favs.iter().collect();
    names.sort();
    let text: String = names.iter().map(|n| format!("{}\n", n)).collect();
    std::fs::write(&path, text).map_err(|e| e.to_string())
}

fn load(pw: &str) -> Data {
    let mut active_names = list_dir("/var/service");
    active_names.sort();
    let active_set: HashSet<&str> = active_names.iter().map(|s| s.as_str()).collect();

    let mut inactive_names: Vec<String> = list_dir("/etc/sv")
        .into_iter()
        .filter(|n| !active_set.contains(n.as_str()) && Path::new("/etc/sv").join(n).is_dir())
        .collect();
    inactive_names.sort();

    let favs = load_favs();
    let mut active: Vec<Svc> = active_names.iter().map(|n| make_svc(pw, "/var/service", n)).collect();
    let mut inactive: Vec<Svc> = inactive_names.iter().map(|n| make_svc(pw, "/etc/sv", n)).collect();
    for s in active.iter_mut().chain(inactive.iter_mut()) {
        s.fav = favs.contains(&s.name);
    }
    // избранное сверху своей половины, внутри — по алфавиту
    let key = |s: &Svc| (!s.fav, s.name.clone());
    active.sort_by_key(key);
    inactive.sort_by_key(key);
    Data { active, inactive }
}

// ── fonts & theme ─────────────────────────────────────────────────────────────

fn setup_fonts_and_theme(cc: &eframe::CreationContext<'_>) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "terminus".into(),
        egui::FontData::from_static(include_bytes!("../assets/Terminus__TTF__500.ttf")),
    );
    fonts.font_data.insert(
        "terminus_bold".into(),
        egui::FontData::from_static(include_bytes!("../assets/Terminus__TTF__Bold_700.ttf")),
    );
    fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "terminus".into());
    fonts.families.entry(egui::FontFamily::Monospace).or_default().insert(0, "terminus".into());
    cc.egui_ctx.set_fonts(fonts);

    let mut visuals = egui::Visuals::dark();
    let black  = Color32::BLACK;
    let white  = Color32::WHITE;
    let gray05 = Color32::from_gray(12);
    let gray10 = Color32::from_gray(22);
    let gray20 = Color32::from_gray(45);

    visuals.override_text_color = Some(white);
    visuals.panel_fill          = black;
    visuals.window_fill         = black;
    visuals.extreme_bg_color    = black;
    visuals.faint_bg_color      = gray05;

    for w in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        w.bg_fill      = gray10;
        w.weak_bg_fill = gray05;
        w.fg_stroke    = Stroke::new(1.0_f32, white);
        w.bg_stroke    = Stroke::new(1.0_f32, gray20);
    }
    visuals.widgets.hovered.bg_fill   = gray20;
    visuals.widgets.active.bg_fill    = gray20;
    visuals.widgets.hovered.expansion = 0.0;
    visuals.widgets.active.expansion  = 0.0;
    visuals.selection.bg_fill = Color32::from_rgb(0, 80, 160);
    visuals.selection.stroke  = Stroke::new(1.0_f32, white);
    visuals.striped = true;

    cc.egui_ctx.set_visuals(visuals);
}

// ── UI helpers ────────────────────────────────────────────────────────────────

const ROW_H:  f32 = 28.0;
const BTN_H:  f32 = 26.0;
const GAP:    f32 = 8.0;

/// Фиксированная раскладка колонок. Одна и та же для шапки и всех строк.
/// Кнопка «удалить» прижата к правому краю, колонки PID/TIME/COMMAND левее неё.
struct Cols {
    name_x: f32,
    act_x:  f32, // первая кнопка: запустить/остановить или включить
    pid_x:  f32,
    time_x: f32,
    cmd_x:  f32,
    del_x:  f32, // удалить, у правого края
}

fn cols(left: f32, right: f32) -> Cols {
    let del_x  = right - 6.0 - BTN_W;
    let time_x = del_x - 16.0 - TIME_W;
    let cmd_x  = time_x - GAP - CMD_W;
    let pid_x  = cmd_x - GAP - PID_W;
    let name_x = left + 64.0; // после звезды и STATUS
    let act_x  = name_x + NAME_W + GAP;
    Cols { name_x, act_x, pid_x, time_x, cmd_x, del_x }
}

/// Кнопка в стиле приложения: серый фон, цветной текст; при наведении/выборе заливается цветом.
fn btn_at(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    salt: (&str, &str),
    label: &str,
    color: Color32,
    hint: &str,
    selected: bool,
) -> bool {
    let resp = ui.interact(rect, ui.id().with(salt), Sense::click());
    let hovered = resp.hovered() || selected;
    let (fill, text) = if hovered { (color, Color32::BLACK) } else { (BTN_BG, color) };
    let p = ui.painter();
    p.rect(rect, 2.0_f32, fill, Stroke::new(1.0_f32, color));
    p.text(rect.center(), Align2::CENTER_CENTER, label, FontId::monospace(13.0), text);
    if selected {
        p.rect_stroke(rect.expand(2.0_f32), 2.0_f32, Stroke::new(1.5_f32, color));
    }
    resp.on_hover_text(hint).clicked()
}

fn status_icon(p: &egui::Painter, c: egui::Pos2, running: bool) {
    let color = if running { GREEN } else { RED_X };
    let s = Stroke::new(2.0_f32, color);
    if running {
        p.line_segment([egui::pos2(c.x - 5.0, c.y + 1.0), egui::pos2(c.x - 1.0, c.y + 5.0)], s);
        p.line_segment([egui::pos2(c.x - 1.0, c.y + 5.0), egui::pos2(c.x + 6.0, c.y - 4.0)], s);
    } else {
        p.line_segment([egui::pos2(c.x - 4.0, c.y - 4.0), egui::pos2(c.x + 4.0, c.y + 4.0)], s);
        p.line_segment([egui::pos2(c.x + 4.0, c.y - 4.0), egui::pos2(c.x - 4.0, c.y + 4.0)], s);
    }
}

/// Звезда «избранное»: залитая жёлтая или серая обводка.
fn draw_star(p: &egui::Painter, c: egui::Pos2, filled: bool) {
    let mut pts = Vec::with_capacity(10);
    for i in 0..10 {
        let r = if i % 2 == 0 { 7.0_f32 } else { 3.0_f32 };
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
        pts.push(egui::pos2(c.x + r * a.cos(), c.y + r * a.sin()));
    }
    let shape = if filled {
        egui::epaint::PathShape {
            points: pts,
            closed: true,
            fill: YELLOW,
            stroke: Stroke::new(1.0_f32, YELLOW).into(),
        }
    } else {
        egui::epaint::PathShape {
            points: pts,
            closed: true,
            fill: Color32::TRANSPARENT,
            stroke: Stroke::new(1.5_f32, STAR_OFF).into(),
        }
    };
    p.add(egui::Shape::Path(shape));
}

/// Жёлтая обводка вокруг звезды (подсветка выбора стрелками).
fn draw_star_outline(p: &egui::Painter, c: egui::Pos2, color: Color32) {
    let mut pts = Vec::with_capacity(11);
    for i in 0..10 {
        let r = if i % 2 == 0 { 11.0_f32 } else { 5.5_f32 };
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
        pts.push(egui::pos2(c.x + r * a.cos(), c.y + r * a.sin()));
    }
    p.add(egui::Shape::closed_line(pts, Stroke::new(1.5_f32, color)));
}

fn header_row(ui: &mut egui::Ui, l: &Lang) {
    let (row, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 16.0), Sense::hover());
    let c = cols(row.left(), row.right());
    let cy = row.center().y;
    let p = ui.painter();
    let f = FontId::monospace(11.0);
    let g = Color32::WHITE;
    p.text(egui::pos2(row.left() + 14.0, cy), Align2::LEFT_CENTER, l.hdr_status, f.clone(), g);
    p.text(egui::pos2(c.name_x, cy), Align2::LEFT_CENTER, l.hdr_service, f.clone(), g);
    p.text(egui::pos2(c.pid_x, cy), Align2::LEFT_CENTER, l.hdr_pid, f.clone(), g);
    p.text(egui::pos2(c.time_x, cy), Align2::LEFT_CENTER, l.hdr_time, f.clone(), g);
    p.text(egui::pos2(c.cmd_x, cy), Align2::LEFT_CENTER, l.hdr_command, f, g);
}

/// Одна строка сервиса. `sel` — выбранная клавиатурой колонка кнопки (если строка выбрана).
fn draw_row(
    ui: &mut egui::Ui,
    l: &Lang,
    s: &Svc,
    inactive: bool,
    busy: bool,
    sel: Option<usize>,
    scroll: bool,
    out: &mut Option<Action>,
) {
    let (row, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_H), Sense::hover());
    if scroll && sel.is_some() {
        resp.scroll_to_me(None);
    }
    let c = cols(row.left(), row.right());
    let cy = row.center().y;
    let is_sel = |i: usize| sel == Some(i);
    let act_rect = |x: f32| {
        egui::Rect::from_min_size(egui::pos2(x, cy - BTN_H / 2.0), egui::vec2(BTN_W, BTN_H))
    };

    // звезда слева от статуса
    let star_c = egui::pos2(row.left() + 12.0, cy);
    draw_star(ui.painter(), star_c, s.fav);
    let star_rect = egui::Rect::from_center_size(star_c, egui::vec2(18.0, 18.0));
    if is_sel(0) {
        draw_star_outline(ui.painter(), star_c, YELLOW);
    }
    if !busy {
        let r = ui.interact(star_rect, ui.id().with((&s.name, "fav")), Sense::click());
        if r.on_hover_text(l.hint_fav).clicked() {
            *out = Some(Action::Favorite(s.name.clone()));
        }
    }

    status_icon(ui.painter(), egui::pos2(row.left() + 36.0, cy), s.running);
    ui.painter().text(
        egui::pos2(c.name_x, cy), Align2::LEFT_CENTER, &s.name,
        FontId::monospace(13.0), Color32::WHITE,
    );

    if busy {
        ui.put(
            egui::Rect::from_min_size(egui::pos2(c.act_x, cy - 8.0), egui::vec2(16.0, 16.0)),
            egui::Spinner::new(),
        );
        return;
    }

    if inactive {
        if btn_at(ui, act_rect(c.act_x), (&s.name, "enable"), l.btn_enable, BLUE, l.hint_enable, is_sel(1)) {
            *out = Some(Action::Link(s.name.clone()));
        }
        if btn_at(ui, act_rect(c.del_x), (&s.name, "delete"), l.btn_remove, PUNCH, l.hint_delete, is_sel(2)) {
            *out = Some(Action::Delete(s.name.clone()));
        }
    } else {
        let (label, color, hint) = if s.running {
            (l.btn_stop, KINOVAR, l.hint_stop)
        } else {
            (l.btn_start, GREEN, l.hint_start)
        };
        if btn_at(ui, act_rect(c.act_x), (&s.name, "toggle"), label, color, hint, is_sel(1)) {
            *out = Some(Action::Toggle {
                name: s.name.clone(),
                path: s.path.clone(),
                running: s.running,
            });
        }
        if btn_at(ui, act_rect(c.del_x), (&s.name, "unlink"), l.btn_disable, PUNCH, l.hint_remove, is_sel(2)) {
            *out = Some(Action::Unlink(s.name.clone()));
        }
    }

    if !s.command_full.is_empty() {
        let cmd_rect = egui::Rect::from_min_size(
            egui::pos2(c.cmd_x, cy - ROW_H / 2.0), egui::vec2(CMD_W, ROW_H));
        ui.interact(cmd_rect, ui.id().with((&s.name, "cmd")), Sense::hover())
            .on_hover_text(&s.command_full);
    }

    let p = ui.painter();
    if !inactive {
        p.text(egui::pos2(c.pid_x, cy), Align2::LEFT_CENTER, &s.pid,
            FontId::monospace(12.0), PID_COL);
        p.text(egui::pos2(c.time_x, cy), Align2::LEFT_CENTER, &s.time,
            FontId::monospace(12.0), Color32::GRAY);
    }
    p.text(egui::pos2(c.cmd_x, cy), Align2::LEFT_CENTER, &s.command,
        FontId::monospace(12.0), CMD_COL);
}

// ── actions ───────────────────────────────────────────────────────────────────

enum Action {
    Toggle { name: String, path: String, running: bool }, // sv up / sv down
    Unlink(String), // rm /var/service/<n>
    Link(String),   // ln -s /etc/sv/<n> /var/service/<n>
    Delete(String), // rm -rf /etc/sv/<n>
    Add(String),    // создать /etc/sv/<n>/run и открыть папку
    Favorite(String), // переключить избранное (~/.local/sv)
}

impl Action {
    fn name(&self) -> &str {
        match self {
            Action::Toggle { name, .. } => name.as_str(),
            Action::Unlink(n) | Action::Link(n) | Action::Delete(n) | Action::Add(n)
            | Action::Favorite(n) => n.as_str(),
        }
    }
}

fn add_service(pw: &str, name: &str) -> Result<(), String> {
    let dir = format!("/etc/sv/{}", name);
    let script = format!(
        "mkdir -p '{d}' && printf '%s\\n' '#!/bin/sh' 'exec 2>&1' '' > '{d}/run' && chmod 755 '{d}/run'",
        d = dir
    );
    sudo_ok(pw, &["sh", "-c", &script])?;
    // файловый менеджер по умолчанию, от имени пользователя
    let _ = Command::new("xdg-open").arg(&dir).spawn();
    Ok(())
}

/// Действие для строки (row) из списка, с учётом того, что сверху идут активные.
fn row_action(data: &Data, row: usize, col: usize) -> Option<Action> {
    let na = data.active.len();
    if row < na {
        let s = &data.active[row];
        return match col {
            0 => Some(Action::Favorite(s.name.clone())),
            1 => Some(Action::Toggle { name: s.name.clone(), path: s.path.clone(), running: s.running }),
            2 => Some(Action::Unlink(s.name.clone())),
            _ => None,
        };
    }
    if row == na {
        return None; // заголовок /etc/sv
    }
    let i = row - na - 1;
    let s = data.inactive.get(i)?;
    match col {
        0 => Some(Action::Favorite(s.name.clone())),
        1 => Some(Action::Link(s.name.clone())),
        2 => Some(Action::Delete(s.name.clone())),
        _ => None,
    }
}

fn row_cols(na: usize, row: usize) -> usize {
    if row == na { 1 } else { 3 } // заголовок /etc/sv: один «столбец»; строки: звезда, кнопка, удалить
}

// ── app ───────────────────────────────────────────────────────────────────────

#[derive(PartialEq)]
enum Screen { Auth, Main }

struct App {
    lang:         &'static Lang,
    screen:       Screen,
    data:         Arc<Mutex<Data>>,
    loading:      Arc<Mutex<bool>>,
    busy:         Arc<Mutex<HashSet<String>>>,
    msg:          Arc<Mutex<Option<String>>>,
    last_load:    Instant,

    password:     String,
    sudo_error:   Option<String>,
    checking:     bool,
    auth_pending: Arc<Mutex<Option<bool>>>,

    expanded:     bool,            // раскрыт ли список /etc/sv
    dlg:          Option<String>,  // Some = диалог «добавить» открыт, внутри имя
    dlg_err:      Option<String>,
    dlg_first:    bool,
    dlg_sel:      usize, // фокус в диалоге: 0 — поле, 1 — «Добавить», 2 — «Отмена»
    confirm:      Option<Action>, // опасное действие, ждёт подтверждения
    confirm_sel:  usize,          // 0 — «Отмена» (по умолчанию), 1 — «Удалить»

    kb_row:       usize,
    kb_col:       usize,
    kb_active:    bool, // подсветка появляется только после первой стрелки
    kb_moved:     bool, // курсор сдвинулся стрелкой в этом кадре: прокрутить к нему
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_fonts_and_theme(cc);
        Self {
            lang:         detect_lang(),
            screen:       Screen::Auth,
            data:         Arc::new(Mutex::new(Data::default())),
            loading:      Arc::new(Mutex::new(false)),
            busy:         Arc::new(Mutex::new(HashSet::new())),
            msg:          Arc::new(Mutex::new(None)),
            last_load:    Instant::now() - Duration::from_secs(999),
            password:     String::new(),
            sudo_error:   None,
            checking:     false,
            auth_pending: Arc::new(Mutex::new(None)),
            expanded:     false,
            dlg:          None,
            dlg_err:      None,
            dlg_first:    false,
            dlg_sel:      0,
            confirm:      None,
            confirm_sel:  0,
            kb_row:       0,
            kb_col:       0,
            kb_active:    false,
            kb_moved:     false,
        }
    }

    fn reload(&mut self) {
        {
            let mut l = self.loading.lock().unwrap();
            if *l { return; }
            *l = true;
        }
        self.last_load = Instant::now();
        let data    = Arc::clone(&self.data);
        let loading = Arc::clone(&self.loading);
        let pw      = self.password.clone();
        thread::spawn(move || {
            let d = load(&pw);
            *data.lock().unwrap()    = d;
            *loading.lock().unwrap() = false;
        });
    }

    /// Мгновенное переключение избранного: звезда и порядок меняются сразу,
    /// запись в ~/.local/sv идёт в фоне.
    fn toggle_favorite_now(&self, name: &str) {
        {
            let mut guard = self.data.lock().unwrap();
            let data = &mut *guard;
            for s in data.active.iter_mut().chain(data.inactive.iter_mut()) {
                if s.name == name {
                    s.fav = !s.fav;
                }
            }
            let key = |s: &Svc| (!s.fav, s.name.clone());
            data.active.sort_by_key(key);
            data.inactive.sort_by_key(key);
        }
        let name = name.to_string();
        thread::spawn(move || {
            let mut favs = load_favs();
            if !favs.remove(&name) {
                favs.insert(name);
            }
            let _ = save_favs(&favs);
        });
    }

    fn dispatch(&self, action: Action) {
        let pw      = self.password.clone();
        let busy    = Arc::clone(&self.busy);
        let data    = Arc::clone(&self.data);
        let loading = Arc::clone(&self.loading);
        let msg     = Arc::clone(&self.msg);

        thread::spawn(move || {
            let key = action.name().to_string();
            busy.lock().unwrap().insert(key.clone());

            let res: Result<(), String> = match action {
                Action::Toggle { path, running, .. } => {
                    sudo_ok(&pw, &["sv", if running { "down" } else { "up" }, &path]).map(|_| ())
                }
                Action::Unlink(n) => sudo_ok(&pw, &["rm", "-f", &format!("/var/service/{}", n)]).map(|_| ()),
                Action::Link(n)   => sudo_ok(&pw, &[
                    "ln", "-s", &format!("/etc/sv/{}", n), &format!("/var/service/{}", n),
                ]).map(|_| ()),
                Action::Delete(n) => sudo_ok(&pw, &["rm", "-rf", &format!("/etc/sv/{}", n)]).map(|_| ()),
                Action::Add(n)    => add_service(&pw, &n),
                Action::Favorite(n) => {
                    let mut favs = load_favs();
                    if !favs.remove(&n) {
                        favs.insert(n);
                    }
                    save_favs(&favs)
                }
            };
            *msg.lock().unwrap() = res.err();

            busy.lock().unwrap().remove(&key);
            *loading.lock().unwrap() = true;
            let d = load(&pw);
            *data.lock().unwrap()    = d;
            *loading.lock().unwrap() = false;
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(500));
        let l = self.lang;

        // ── Экран авторизации ─────────────────────────────────────────────────
        if self.screen == Screen::Auth {
            let res = *self.auth_pending.lock().unwrap();
            if let Some(ok) = res {
                *self.auth_pending.lock().unwrap() = None;
                self.checking = false;
                if ok {
                    self.screen = Screen::Main;
                    self.sudo_error = None;
                    self.reload();
                } else {
                    self.sudo_error = Some(l.wrong_password.into());
                    self.password.clear();
                }
            }

            let checking = self.checking;
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add_space(80.0);
                ui.vertical_centered(|ui| {
                    ui.heading(RichText::new(l.title).size(22.0));
                    ui.add_space(24.0);
                    ui.label(RichText::new(l.sudo_prompt).size(14.0).family(egui::FontFamily::Monospace));
                    ui.add_space(8.0);
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.password)
                            .password(true)
                            .hint_text(l.sudo_placeholder)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(260.0),
                    );
                    resp.request_focus();
                    if let Some(ref err) = self.sudo_error {
                        ui.add_space(6.0);
                        ui.colored_label(RED_X,
                            RichText::new(err.as_str()).size(13.0).family(egui::FontFamily::Monospace));
                    }
                    ui.add_space(14.0);
                    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
                    let btn = ui.add_enabled(
                        !checking,
                        egui::Button::new(RichText::new(l.unlock).size(13.0).family(egui::FontFamily::Monospace))
                            .min_size(egui::vec2(100.0, 28.0)),
                    );
                    if (btn.clicked() || enter) && !checking {
                        if self.password.is_empty() {
                            self.sudo_error = Some(l.enter_password.into());
                        } else {
                            self.checking = true;
                            self.sudo_error = None;
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

        // ── автообновление каждые 5с ──────────────────────────────────────────
        if self.last_load.elapsed() > Duration::from_secs(5) && !*self.loading.lock().unwrap() {
            self.reload();
        }

        let data = self.data.lock().unwrap().clone();
        let busy = self.busy.lock().unwrap().clone();
        let na = data.active.len();
        let ni = data.inactive.len();
        // строки: активные, заголовок /etc/sv (индекс na), [выключенные, «добавить»]
        let total = if na == 0 && ni == 0 {
            0
        } else {
            na + 1 + if self.expanded { ni + 1 } else { 0 }
        };
        if total == 0 {
            self.kb_row = 0;
        } else if self.kb_row >= total {
            self.kb_row = total - 1;
        }

        let mut pending: Option<Action> = None;
        let mut open_dlg = false;

        // ── клавиатура (когда диалог закрыт) ──────────────────────────────────
        if self.dlg.is_none() && self.confirm.is_none() && total > 0 {
            let (up, down, left, right, enter, q) = ctx.input(|i| (
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::ArrowLeft),
                i.key_pressed(egui::Key::ArrowRight),
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Q)
                    || i.events.iter().any(|e| matches!(e,
                        egui::Event::Text(t) if t == "й" || t == "Й")),
            ));
            if q {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            let arrow = up || down || left || right;
            if arrow && !self.kb_active {
                // первая стрелка: только включаем подсветку, курсор в начало
                self.kb_active = true;
                self.kb_row = 0;
                self.kb_col = 0;
            } else if self.kb_active {
                if down && self.kb_row + 1 < total {
                    self.kb_row += 1;
                }
                if up && self.kb_row > 0 {
                    self.kb_row -= 1;
                }
                let cols = if self.expanded && self.kb_row == total - 1 { 1 } else { row_cols(na, self.kb_row) };
                if left  && self.kb_col > 0 { self.kb_col -= 1; }
                if right && self.kb_col + 1 < cols { self.kb_col += 1; }
                if self.kb_col >= cols { self.kb_col = cols - 1; }
            }

            if arrow {
                self.kb_moved = true;
            }

            if enter && self.kb_active {
                if self.kb_row == na {
                    // заголовок /etc/sv: раскрыть/свернуть
                    self.expanded = !self.expanded;
                } else if self.expanded && self.kb_row == total - 1 {
                    open_dlg = true;
                } else if let Some(a) = row_action(&data, self.kb_row, self.kb_col) {
                    if !busy.contains(a.name()) {
                        pending = Some(a);
                    }
                }
            }
        }

        // ── диалог «добавить сервис» ──────────────────────────────────────────
        if open_dlg {
            self.dlg = Some(String::new());
            self.dlg_err = None;
            self.dlg_first = true;
            self.dlg_sel = 0;
        }
        if let Some(mut name) = self.dlg.take() {
            let first = self.dlg_first;
            let mut submit = false;
            let mut close = false;
            let mut open = true;
            let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
            let esc   = ctx.input(|i| i.key_pressed(egui::Key::Escape));
            let valid = valid_name(&name);
            let err = self.dlg_err.clone();

            // Стрелки перехватываем до того, как их заберёт поле ввода.
            let te_id = egui::Id::new("dlg_name_field");
            let mut sel = self.dlg_sel;
            let (kd, ku, kl, kr) = ctx.input_mut(|i| (
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                sel != 0 && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft),
                sel != 0 && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight),
            ));
            if kd && sel == 0 { sel = 1; }          // поле → «Добавить»
            if ku && sel != 0 { sel = 0; }          // кнопки → поле
            if kl && sel == 2 { sel = 1; }          // «Отмена» → «Добавить»
            if kr && sel == 1 { sel = 2; }          // «Добавить» → «Отмена»
            if sel != 0 {
                // уводим фокус с поля, чтобы буквы не печатались в него
                ctx.memory_mut(|m| m.surrender_focus(te_id));
            }
            let mut clicked_add = false;
            let mut clicked_cancel = false;

            egui::Window::new(l.dlg_title)
                .collapsible(false)
                .resizable(false)
                .default_width(360.0)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(l.dlg_name).size(13.0));
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut name)
                                .desired_width(200.0)
                                .hint_text("myservice")
                                .font(egui::TextStyle::Monospace)
                                .id(te_id),
                        );
                        if first || sel == 0 { r.request_focus(); }
                    });
                    if !name.is_empty() && !valid {
                        ui.colored_label(RED_X, RichText::new(l.dlg_bad_name).size(12.0));
                    }
                    if let Some(e) = &err {
                        ui.colored_label(RED_X, RichText::new(e.as_str()).size(12.0));
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let add_resp = ui.add_enabled(valid, egui::Button::new(RichText::new(l.dlg_add).size(13.0)
                            .color(GREEN)));
                        if sel == 1 {
                            ui.painter().rect_stroke(add_resp.rect.expand(2.0_f32), 3.0_f32, Stroke::new(1.5_f32, YELLOW));
                        }
                        if add_resp.clicked() { clicked_add = true; }
                        let cancel_resp = ui.button(RichText::new(l.dlg_cancel).size(13.0));
                        if sel == 2 {
                            ui.painter().rect_stroke(cancel_resp.rect.expand(2.0_f32), 3.0_f32, Stroke::new(1.5_f32, YELLOW));
                        }
                        if cancel_resp.clicked() { clicked_cancel = true; }
                    });
                });

            if clicked_add || (enter && sel != 2 && valid) { submit = true; }
            if clicked_cancel || (enter && sel == 2) { close = true; }
            self.dlg_sel = sel;

            self.dlg_first = false;
            if submit {
                if Path::new("/etc/sv").join(&name).exists() {
                    self.dlg_err = Some(l.dlg_exists.into());
                    self.dlg = Some(name);
                } else {
                    self.dlg_err = None;
                    self.dispatch(Action::Add(name));
                }
            } else if close || !open || esc {
                self.dlg_err = None;
            } else {
                self.dlg = Some(name);
            }
        }

        // ── ошибка и спиннер: плавают поверх, без фоновой панели ──────────────
        let msg = self.msg.lock().unwrap().clone();
        if let Some(m) = &msg {
            egui::Area::new(egui::Id::new("sv_msg"))
                .anchor(Align2::LEFT_BOTTOM, [12.0_f32, -8.0_f32])
                .show(ctx, |ui| {
                    ui.colored_label(RED_X, RichText::new(m.as_str()).monospace().size(12.0));
                });
        }
        if *self.loading.lock().unwrap() {
            egui::Area::new(egui::Id::new("sv_spinner"))
                .anchor(Align2::RIGHT_BOTTOM, [-12.0_f32, -8.0_f32])
                .show(ctx, |ui| {
                    ui.spinner();
                });
        }

        // ── основной список ───────────────────────────────────────────────────
        let expanded = self.expanded;
        let kb_row = self.kb_row;
        let kb_col = self.kb_col;
        let kb_active = self.kb_active;
        let scroll_now = self.kb_moved;
        self.kb_moved = false;
        let mut clicked: Option<Action> = None;
        let mut toggle_expanded = false;
        let mut add_clicked = false;

        egui::CentralPanel::default().show(ctx, |ui| {
            if na == 0 && ni == 0 {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new(l.no_services).color(egui::Color32::GRAY));
                });
                return;
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                // ── верх: /var/service ──
                ui.add(egui::Label::new(
                    RichText::new(l.active_hdr).monospace().size(24.0).color(egui::Color32::WHITE)));
                header_row(ui, l);
                ui.separator();
                for (i, s) in data.active.iter().enumerate() {
                    let sel = if kb_active && kb_row == i { Some(kb_col) } else { None };
                    draw_row(ui, l, s, false, busy.contains(&s.name), sel, scroll_now, &mut clicked);
                    ui.separator();
                }

                ui.add_space(10.0);
                ui.separator();

                // ── низ: /etc/sv (раскрывается по клику) ──
                let arrow = if expanded { "▲" } else { "▼" };
                let hdr = ui.add(egui::Label::new(
                    RichText::new(format!("{}  {}", l.inactive_hdr, arrow))
                        .monospace().size(22.0).color(Color32::WHITE))
                    .sense(Sense::click()))
                    .on_hover_cursor(CursorIcon::PointingHand);
                if hdr.clicked() {
                    toggle_expanded = true;
                }
                if kb_active && kb_row == na {
                    ui.painter().rect_stroke(
                        hdr.rect.expand(2.0_f32), 2.0_f32,
                        Stroke::new(1.5_f32, Color32::from_gray(190)));
                    if scroll_now {
                        hdr.scroll_to_me(None);
                    }
                }

                if expanded {
                    ui.add_space(4.0);
                    header_row(ui, l);
                    ui.separator();
                    for (j, s) in data.inactive.iter().enumerate() {
                        let row = na + 1 + j;
                        let sel = if kb_active && kb_row == row { Some(kb_col) } else { None };
                        draw_row(ui, l, s, true, busy.contains(&s.name), sel, scroll_now, &mut clicked);
                        ui.separator();
                    }

                    ui.add_space(6.0);
                    let row = total - 1;
                    let sel = kb_active && kb_row == row;
                    ui.horizontal(|ui| {
                        ui.add_space(4.0);
                        let (rect, add_resp) = ui.allocate_exact_size(
                            egui::vec2(200.0, BTN_H), Sense::hover());
                        if sel && scroll_now {
                            add_resp.scroll_to_me(None);
                        }
                        if btn_at(ui, rect, ("add", "add"), l.add_service, GREEN, l.add_hint, sel) {
                            add_clicked = true;
                        }
                    });
                }
            });
        });

        if toggle_expanded {
            self.expanded = !self.expanded;
        }
        if add_clicked {
            self.dlg = Some(String::new());
            self.dlg_err = None;
            self.dlg_first = true;
            self.dlg_sel = 0;
        }

        // опасные действия (удаление) сначала спрашиваем, остальные — сразу
        let mut to_run: Option<Action> = None;
        if let Some(a) = clicked.or(pending) {
            // «удалить» (rm -rf) — всегда спрашиваем; «отключить» — только если сервис работает
            let ask = match &a {
                Action::Delete(_) => true,
                Action::Unlink(n) => data.active.iter().any(|s| s.name == *n && s.running),
                _ => false,
            };
            if ask {
                if self.confirm.is_none() && !busy.contains(a.name()) {
                    self.confirm = Some(a);
                    self.confirm_sel = 0;
                }
            } else {
                to_run = Some(a);
            }
        }

        // ── окно подтверждения ────────────────────────────────────────────────
        if let Some(a) = self.confirm.take() {
            let (kl, kr, enter, esc) = ctx.input_mut(|i| (
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft),
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
            ));
            let mut sel = self.confirm_sel;
            if kl && sel == 1 { sel = 0; }
            if kr && sel == 0 { sel = 1; }
            let mut yes = false;
            let mut no = false;

            let (question, name, yes_label) = match &a {
                Action::Unlink(n) => (l.confirm_unlink, n.clone(), l.btn_disable),
                Action::Delete(n) => (l.confirm_delete, n.clone(), l.btn_remove),
                _ => ("", String::new(), ""),
            };

            egui::Window::new(l.confirm_title)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(RichText::new(question).size(13.0));
                    ui.label(RichText::new(name.as_str()).monospace().size(14.0).color(RED_X));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let cancel = ui.button(RichText::new(l.dlg_cancel).size(13.0));
                        if sel == 0 {
                            ui.painter().rect_stroke(cancel.rect.expand(2.0_f32), 3.0_f32, Stroke::new(1.5_f32, YELLOW));
                        }
                        if cancel.clicked() { no = true; }
                        let del = ui.add(egui::Button::new(RichText::new(yes_label).size(13.0).color(PUNCH)));
                        if sel == 1 {
                            ui.painter().rect_stroke(del.rect.expand(2.0_f32), 3.0_f32, Stroke::new(1.5_f32, YELLOW));
                        }
                        if del.clicked() { yes = true; }
                    });
                });

            if enter { if sel == 1 { yes = true; } else { no = true; } }
            if esc { no = true; }
            self.confirm_sel = sel;

            if yes {
                to_run = Some(a);
            } else if !no {
                self.confirm = Some(a); // окно остаётся открытым
            }
        }

        if let Some(a) = to_run {
            if let Action::Favorite(ref n) = a {
                self.toggle_favorite_now(n);
            } else if !busy.contains(a.name()) {
                self.dispatch(a);
            }
        }
    }
}

fn main() -> eframe::Result<()> {
    let title = detect_lang().title;
    eframe::run_native(
        title,
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title(title)
                .with_inner_size([720.0_f32, 480.0_f32])
                .with_min_inner_size([660.0_f32, 300.0_f32]),
            ..Default::default()
        },
        Box::new(|cc| Box::new(App::new(cc))),
    )
}
