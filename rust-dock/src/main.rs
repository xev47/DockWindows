#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

mod active_apps;
mod file_dialog;
mod startup;
mod system_controls;
mod weather;
mod window_thumbnails;

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::{self, Align2, Color32, TextureHandle};
use serde::{Deserialize, Serialize};

const WINDOW_WIDTH: f32 = 900.0;
const WINDOW_HEIGHT: f32 = 170.0;
const DOCK_BOTTOM_MARGIN: f32 = 8.0;
const DOCK_EDGE_MARGIN: f32 = 8.0;
const VERTICAL_DOCK_WIDTH: f32 = 220.0;
const MIN_WINDOW_WIDTH: f32 = 100.0;
const MIN_WINDOW_HEIGHT: f32 = 100.0;
const APP_NAME: &str = "Dock Windows";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_ICON_SIZE: f32 = 38.0;
const MIN_ICON_SIZE: f32 = 24.0;
const MAX_ICON_SIZE: f32 = 48.0;
const DEFAULT_VERTICAL_PADDING: u8 = 2;
const MIN_VERTICAL_PADDING: u8 = 2;
const MAX_VERTICAL_PADDING: u8 = 30;
const DEFAULT_OPACITY: u8 = 100;
const DEFAULT_ICON_OPACITY: u8 = 100;
const MIN_OPACITY: u8 = 20;
const MAX_OPACITY: u8 = 100;
const ICON_CELL_PADDING: f32 = 14.0;
const FRAME_HORIZONTAL_MARGIN: f32 = 16.0;
const DOCK_FRAME_STROKE_WIDTH: f32 = 1.0;
const SEARCH_FIELD_HEIGHT: f32 = 34.0;
const SEARCH_FIELD_WIDTH: f32 = 190.0;
const SEARCH_RESULTS_MAX_HEIGHT: f32 = 196.0;
const SEARCH_RESULTS_MAX_COUNT: usize = 7;
const SEARCH_RESULT_ROW_HEIGHT: f32 = 28.0;
const SEARCH_RESULTS_FRAME_PADDING: f32 = 12.0;
const SEARCH_RESULTS_ROW_SPACING: f32 = 2.0;
const SETTINGS_MENU_EXTRA_HEIGHT: f32 = 440.0;
const SETTINGS_PANEL_WIDTH: f32 = 380.0;
const SETTINGS_PANEL_HEIGHT: f32 = 500.0;
const SETUP_WINDOW_WIDTH: f32 = 600.0;
const SETUP_WINDOW_HEIGHT: f32 = 500.0;
const CONFIRMATION_EXTRA_HEIGHT: f32 = 220.0;
const CONFIRMATION_MIN_WIDTH: f32 = 340.0;
const POPUP_SCREEN_MARGIN: f32 = 8.0;
const POPUP_GAP: f32 = 8.0;
const WINDOW_PREVIEW_CARD_WIDTH: f32 = 210.0;
const WINDOW_PREVIEW_CARD_HEIGHT: f32 = 112.0;
const WINDOW_PREVIEW_CARD_LABEL_HEIGHT: f32 = 40.0;
const WINDOW_PREVIEW_FIXED_HEIGHT: f32 = 96.0;
const WINDOW_PREVIEW_ROW_SPACING: f32 = 8.0;
const WINDOW_PREVIEW_MAX_COLUMNS: usize = 3;
const DEFAULT_WEATHER_CITY: &str = "Lille";
const WEATHER_REFRESH_INTERVAL: Duration = Duration::from_secs(15 * 60);
const WEATHER_RETRY_INTERVAL: Duration = Duration::from_secs(60);
const LUNAR_CYCLE_SECONDS: f64 = 29.530588853 * 24.0 * 60.0 * 60.0;
const KNOWN_NEW_MOON_UNIX_SECONDS: i64 = 947_182_440;
const CLOCK_DISPLAY_WIDTH: f32 = 48.0;
const WEATHER_DISPLAY_WIDTH: f32 = 72.0;
const WEATHER_STACK_HEIGHT: f32 = 52.0;
const CLOCK_STACK_HEIGHT: f32 = 20.0;
const SYSTEM_CONTROL_SIZE: f32 = 36.0;

// Palette des menus : un verre sombre et froid, avec un accent bleu-lavande
// repris des pastilles de l'icone de l'application.
const MENU_ICON_SETTINGS: &str = "⚙";
const MENU_ICON_REMOVE: &str = "🗑";
const MENU_ICON_MOVE: &str = "↔";
const MENU_ICON_PIN: &str = "📌";
const MENU_ICON_WINDOWS: &str = "🖥";
const MENU_ICON_CLOSE: &str = "×";
const MENU_ICON_HIDE: &str = "🚫";
const MENU_ICON_SHOW: &str = "👁";
const MENU_ICON_RESTORE_ALL: &str = "⟳";
const MENU_ICON_QUIT: &str = "🚪";
const MENU_ICON_GROUP: &str = "📁";
const MENU_ICON_NEW_GROUP: &str = "➕";
const MENU_ICON_RENAME: &str = "✏";
const MENU_ICON_MENU: &str = "☰";

fn menu_text_color() -> Color32 {
    Color32::from_rgb(226, 230, 238)
}

fn menu_muted_color() -> Color32 {
    Color32::from_rgb(138, 147, 166)
}

fn menu_accent_color() -> Color32 {
    Color32::from_rgb(112, 141, 240)
}

/// Applique le theme commun a la fenetre principale et au panneau des
/// parametres. Les menus, les infobulles et les fenetres modales en heritent.
fn apply_design(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let mut visuals = egui::Visuals::dark();

    let glass = Color32::from_rgba_unmultiplied(20, 22, 29, 248);
    let hairline = Color32::from_white_alpha(26);

    visuals.panel_fill = glass;
    visuals.window_fill = glass;
    visuals.window_stroke = egui::Stroke::new(1.0, hairline);
    visuals.window_corner_radius = egui::CornerRadius::same(18);
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 16],
        blur: 36,
        spread: 0,
        color: Color32::from_black_alpha(170),
    };
    visuals.menu_corner_radius = egui::CornerRadius::same(14);
    visuals.popup_shadow = egui::epaint::Shadow {
        offset: [0, 12],
        blur: 30,
        spread: 2,
        color: Color32::from_black_alpha(160),
    };
    visuals.extreme_bg_color = Color32::from_rgb(13, 15, 20);
    visuals.faint_bg_color = Color32::from_white_alpha(7);
    visuals.code_bg_color = Color32::from_rgb(34, 38, 48);
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(112, 141, 240, 90);
    visuals.hyperlink_color = menu_accent_color();

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(38, 42, 52);
    visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, menu_text_color());
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(8);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(48, 54, 68);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgba_unmultiplied(112, 141, 240, 46);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, Color32::from_white_alpha(18));
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, Color32::WHITE);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(8);

    visuals.widgets.active.bg_fill = Color32::from_rgb(58, 64, 82);
    visuals.widgets.active.weak_bg_fill = Color32::from_rgba_unmultiplied(112, 141, 240, 74);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, Color32::from_white_alpha(28));
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, Color32::WHITE);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(8);

    visuals.widgets.open = visuals.widgets.hovered;

    visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, Color32::from_white_alpha(16));
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, menu_text_color());
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(8);

    style.visuals = visuals;
    style.spacing.menu_margin = egui::Margin::same(8);
    style.spacing.menu_width = 320.0;
    style.spacing.button_padding = egui::vec2(8.0, 5.0);
    style.spacing.interact_size.y = 26.0;
    style.spacing.slider_width = 150.0;
    ctx.set_style(style);
}

fn main() {
    if let Err(error) = run() {
        show_error(&error.to_string());
    }
}

fn run() -> Result<(), String> {
    let app = DockApp::load().map_err(|error| error.to_string())?;
    let initial_size = initial_dock_window_size(app.config.dock_position);
    let window_level = if app.config.always_on_top {
        egui::WindowLevel::AlwaysOnTop
    } else {
        egui::WindowLevel::Normal
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_icon(application_icon())
            .with_inner_size(initial_size)
            .with_position(initial_dock_position(
                app.config.dock_position,
                initial_size,
            ))
            .with_decorations(false)
            .with_window_level(window_level)
            .with_transparent(true)
            .with_resizable(false),
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |creation_context| {
            apply_design(&creation_context.egui_ctx);
            Ok(Box::new(app))
        }),
    )
    .map_err(|error| error.to_string())
}

fn initial_dock_window_size(position: DockPosition) -> [f32; 2] {
    if position.is_vertical() {
        [VERTICAL_DOCK_WIDTH, WINDOW_HEIGHT]
    } else {
        [WINDOW_WIDTH, WINDOW_HEIGHT]
    }
}

#[cfg(windows)]
fn initial_dock_position(position: DockPosition, size: [f32; 2]) -> egui::Pos2 {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::{
        GetDC, GetDeviceCaps, GetMonitorInfoW, MonitorFromPoint, ReleaseDC, LOGPIXELSX,
        MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };

    let scale = unsafe {
        let device_context = GetDC(std::ptr::null_mut());
        if device_context.is_null() {
            1.0
        } else {
            let dpi = GetDeviceCaps(device_context, LOGPIXELSX as i32);
            ReleaseDC(std::ptr::null_mut(), device_context);
            (dpi as f32 / 96.0).max(1.0)
        }
    };
    let work_area = unsafe {
        let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if monitor.is_null() || GetMonitorInfoW(monitor, &mut info) == 0 {
            None
        } else {
            Some(info.rcWork)
        }
    };
    let Some(work) = work_area else {
        return egui::pos2(100.0, 100.0);
    };
    let work_area = egui::Rect::from_min_max(
        egui::pos2(work.left as f32 / scale, work.top as f32 / scale),
        egui::pos2(work.right as f32 / scale, work.bottom as f32 / scale),
    );
    let [x, y] = dock_viewport_position(position, size[0], size[1], work_area);
    egui::pos2(x, y)
}

#[cfg(not(windows))]
fn initial_dock_position(_position: DockPosition, _size: [f32; 2]) -> egui::Pos2 {
    egui::pos2(100.0, 100.0)
}

/// Version affichee dans les parametres : les publications sont numerotees
/// `majeure.build.0`, le correctif nul n'est pas montre ("v1.28").
fn display_version() -> &'static str {
    short_version(APP_VERSION)
}

fn short_version(version: &str) -> &str {
    version.strip_suffix(".0").unwrap_or(version)
}

fn application_icon() -> egui::IconData {
    egui::IconData {
        rgba: include_bytes!("../assets/dock-windows.rgba").to_vec(),
        width: 256,
        height: 256,
    }
}

#[cfg(windows)]
fn show_error(message: &str) {
    use std::iter::once;
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    let message: Vec<u16> = message.encode_utf16().chain(once(0)).collect();
    let title: Vec<u16> = APP_NAME.encode_utf16().chain(once(0)).collect();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(not(windows))]
fn show_error(message: &str) {
    eprintln!("{message}");
}

#[cfg(windows)]
fn show_info(message: &str) {
    use std::iter::once;
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};

    let message: Vec<u16> = message.encode_utf16().chain(once(0)).collect();
    let title: Vec<u16> = APP_NAME.encode_utf16().chain(once(0)).collect();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

#[cfg(not(windows))]
fn show_info(message: &str) {
    eprintln!("{message}");
}

fn dock_viewport_position(
    position: DockPosition,
    width: f32,
    height: f32,
    work_area: egui::Rect,
) -> [f32; 2] {
    let center = work_area.center();
    match position {
        DockPosition::Bottom => [
            (center.x - width / 2.0).clamp(
                work_area.left(),
                (work_area.right() - width).max(work_area.left()),
            ),
            (work_area.bottom() - height - DOCK_BOTTOM_MARGIN).max(work_area.top()),
        ],
        DockPosition::Top => [
            (center.x - width / 2.0).clamp(
                work_area.left(),
                (work_area.right() - width).max(work_area.left()),
            ),
            work_area.top() + DOCK_EDGE_MARGIN,
        ],
        DockPosition::Left => [
            work_area.left() + DOCK_EDGE_MARGIN,
            (center.y - height / 2.0).clamp(
                work_area.top(),
                (work_area.bottom() - height).max(work_area.top()),
            ),
        ],
        DockPosition::Right => [
            (work_area.right() - width - DOCK_EDGE_MARGIN).max(work_area.left()),
            (center.y - height / 2.0).clamp(
                work_area.top(),
                (work_area.bottom() - height).max(work_area.top()),
            ),
        ],
    }
}

fn dock_area_anchor(position: DockPosition) -> (Align2, egui::Vec2) {
    match position {
        DockPosition::Bottom => (Align2::CENTER_BOTTOM, egui::Vec2::ZERO),
        DockPosition::Top => (Align2::CENTER_TOP, egui::Vec2::ZERO),
        DockPosition::Left => (Align2::LEFT_CENTER, egui::Vec2::ZERO),
        DockPosition::Right => (Align2::RIGHT_CENTER, egui::Vec2::ZERO),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AppLanguage {
    #[default]
    French,
    English,
    Spanish,
}

impl AppLanguage {
    fn code(self) -> &'static str {
        match self {
            Self::French => "fr",
            Self::English => "en",
            Self::Spanish => "es",
        }
    }

    fn native_name(self) -> &'static str {
        match self {
            Self::French => "Français",
            Self::English => "English",
            Self::Spanish => "Español",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DockSide {
    Left,
    #[default]
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DockPosition {
    #[default]
    Bottom,
    Top,
    Left,
    Right,
}

impl DockPosition {
    fn is_vertical(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

fn tr(language: AppLanguage, key: &str) -> &str {
    let french = match key {
        "settings" => "Paramètres",
        "close" => "Fermer",
        "menu_section_application" => "Application",
        "menu_section_organize" => "Rangement",
        "menu_section_dock" => "Dock",
        "settings_section_appearance" => "Apparence",
        "settings_section_content" => "Contenu du dock",
        "settings_section_position" => "Position et couleurs",
        "settings_section_system" => "Système",
        "settings_section_backup" => "Sauvegarde",
        "setup_title" => "Bienvenue dans Dock Windows",
        "setup_subtitle" => "Choisissez la langue du dock et les raccourcis à importer.",
        "setup_language" => "Langue de l'application",
        "setup_shortcuts" => "Raccourcis Windows",
        "setup_taskbar" => "Barre des tâches",
        "setup_desktop" => "Bureau",
        "setup_start_menu" => "Menu Démarrer",
        "setup_summary_one" => "{count} raccourci sera ajouté au dock.",
        "setup_summary_many" => "{count} raccourcis seront ajoutés au dock.",
        "setup_summary_none" => "Aucun raccourci ne sera importé pour le moment.",
        "setup_hint" => "Vous pourrez modifier ces choix à tout moment dans les paramètres.",
        "setup_start" => "Commencer",
        "setup_skip" => "Ignorer",
        "setup_group_default" => "Général",
        "remove_shortcut" => "Retirer du dock",
        "add_current_group" => "Ajouter au groupe courant",
        "add_to_group" => "Ajouter à un groupe",
        "move_to_group" => "Déplacer vers un groupe",
        "drop_into_group" => "Déposer dans un groupe",
        "open_windows" => "Fenêtres ouvertes",
        "close_window" => "Fermer une fenêtre",
        "close_this_window" => "Fermer cette fenêtre",
        "close_all_windows" => "Fermer toutes les fenêtres de l'application",
        "close_denied" => "Windows n'a pas accepté la demande de fermeture.",
        "close_unsupported" => "La fermeture native n'est prise en charge que sous Windows.",
        "power_off" => "Arrêter l'ordinateur",
        "weather_at" => "Météo à",
        "weather_loading_at" => "Chargement de la météo à",
        "weather_request_error" => "Impossible de démarrer la mise à jour",
        "local_time" => "Heure locale",
        "quit" => "Quitter",
        "no_apps_found" => "Aucune application trouvée.",
        "search_error" => "Impossible de rechercher les applications",
        "preview_unavailable" => "Aperçu indisponible",
        "shutdown_title" => "Confirmation d'arrêt",
        "shutdown_question" => "Voulez-vous vraiment arrêter l'ordinateur ?",
        "cancel" => "Annuler",
        "shutdown_now" => "Arrêter maintenant",
        "groups" => "Groupes",
        "new_group" => "Nouveau groupe",
        "delete_group" => "Supprimer",
        "rename_group" => "Renommer",
        "group_name" => "Nom du groupe",
        "create_group_action" => "Créer",
        "rename_group_action" => "Renommer",
        "group_name_empty" => "Le nom du groupe ne peut pas être vide.",
        "group_name_exists" => "Un groupe porte déjà ce nom.",
        "hide_application" => "Masquer cette application",
        "hidden_applications" => "Applications masquées",
        "show_application" => "Réafficher",
        "show_all_applications" => "Tout réafficher",
        "icon_size" => "Taille des icônes",
        "vertical_spacing" => "Espace vertical autour des icônes",
        "background_opacity" => "Opacité du fond",
        "icon_opacity" => "Opacité des icônes",
        "show_clock" => "Afficher l'heure",
        "show_weather" => "Afficher la météo",
        "show_shutdown_button" => "Afficher le bouton d'arrêt",
        "show_wifi" => "Afficher le Wi-Fi",
        "show_volume" => "Afficher le son",
        "show_search" => "Afficher le champ de recherche",
        "wifi_error" => "Impossible d'ouvrir les réseaux Wi-Fi",
        "volume_error" => "Impossible de régler le volume",
        "volume" => "Volume",
        "always_on_top" => "Toujours garder le dock au premier plan",
        "startup" => "Démarrer avec Windows",
        "language" => "Langue",
        "weather_city" => "Ville météo",
        "apply" => "Appliquer",
        "shutdown_position" => "Position du bouton d'arrêt",
        "weather_position" => "Position du bloc météo",
        "dock_position" => "Position du dock",
        "dock_background_color" => "Couleur de fond du dock",
        "bottom" => "En bas",
        "top" => "En haut",
        "left" => "Gauche",
        "right" => "Droite",
        "settings_save_error" => "Erreur d'enregistrement",
        "configuration_backup" => "Sauvegarde de la configuration",
        "export_config" => "Exporter…",
        "import_config" => "Importer…",
        "export_config_title" => "Exporter la configuration",
        "import_config_title" => "Importer la configuration",
        "export_config_done" => "Configuration exportée vers",
        "export_config_error" => "Impossible d'exporter la configuration",
        "import_config_done" => "Configuration importée",
        "import_config_invalid" => "Ce fichier n'est pas une configuration Dock Windows valide.",
        "invalid_drop" => {
            "Certains éléments déposés ne sont pas des fichiers ou dossiers accessibles."
        }
        "launch_error" => "Impossible d'ouvrir",
        "startup_error" => "Impossible de modifier le démarrage automatique",
        "shutdown_error" => "Impossible d'arrêter l'ordinateur",
        "search_hint" => "Rechercher",
        _ => return key,
    };
    match language {
        AppLanguage::French => french,
        AppLanguage::English => match key {
            "settings" => "Settings",
            "close" => "Close",
            "menu_section_application" => "Application",
            "menu_section_organize" => "Organize",
            "menu_section_dock" => "Dock",
            "settings_section_appearance" => "Appearance",
            "settings_section_content" => "Dock content",
            "settings_section_position" => "Position and colors",
            "settings_section_system" => "System",
            "settings_section_backup" => "Backup",
            "setup_title" => "Welcome to Dock Windows",
            "setup_subtitle" => "Choose the dock language and the shortcuts to import.",
            "setup_language" => "Application language",
            "setup_shortcuts" => "Windows shortcuts",
            "setup_taskbar" => "Taskbar",
            "setup_desktop" => "Desktop",
            "setup_start_menu" => "Start menu",
            "setup_summary_one" => "{count} shortcut will be added to the dock.",
            "setup_summary_many" => "{count} shortcuts will be added to the dock.",
            "setup_summary_none" => "No shortcuts will be imported for now.",
            "setup_hint" => "You can change these choices at any time in Settings.",
            "setup_start" => "Get started",
            "setup_skip" => "Skip",
            "setup_group_default" => "General",
            "remove_shortcut" => "Remove from dock",
            "add_current_group" => "Add to current group",
            "add_to_group" => "Add to a group",
            "move_to_group" => "Move to a group",
            "drop_into_group" => "Drop into a group",
            "open_windows" => "Open windows",
            "close_window" => "Close a window",
            "close_this_window" => "Close this window",
            "close_all_windows" => "Close all application windows",
            "close_denied" => "Windows did not accept the close request.",
            "close_unsupported" => "Native closing is only supported on Windows.",
            "power_off" => "Shut down the computer",
            "weather_at" => "Weather in",
            "weather_loading_at" => "Loading weather in",
            "weather_request_error" => "Unable to start the weather update",
            "local_time" => "Local time",
            "quit" => "Quit",
            "no_apps_found" => "No applications found.",
            "search_error" => "Unable to search applications",
            "preview_unavailable" => "Preview unavailable",
            "shutdown_title" => "Shut down confirmation",
            "shutdown_question" => "Are you sure you want to shut down the computer?",
            "cancel" => "Cancel",
            "shutdown_now" => "Shut down now",
            "groups" => "Groups",
            "new_group" => "New group",
            "delete_group" => "Delete",
            "rename_group" => "Rename",
            "group_name" => "Group name",
            "create_group_action" => "Create",
            "rename_group_action" => "Rename",
            "group_name_empty" => "The group name cannot be empty.",
            "group_name_exists" => "A group already has this name.",
            "hide_application" => "Hide this application",
            "hidden_applications" => "Hidden applications",
            "show_application" => "Show",
            "show_all_applications" => "Show all",
            "icon_size" => "Icon size",
            "vertical_spacing" => "Vertical spacing around icons",
            "background_opacity" => "Background opacity",
            "icon_opacity" => "Icon opacity",
            "show_clock" => "Show clock",
            "show_weather" => "Show weather",
            "show_shutdown_button" => "Show shutdown button",
            "show_wifi" => "Show Wi-Fi",
            "show_volume" => "Show sound",
            "show_search" => "Show search field",
            "wifi_error" => "Unable to open Wi-Fi networks",
            "volume_error" => "Unable to adjust volume",
            "volume" => "Volume",
            "always_on_top" => "Keep the dock on top",
            "startup" => "Start with Windows",
            "language" => "Language",
            "weather_city" => "Weather city",
            "apply" => "Apply",
            "shutdown_position" => "Shutdown button position",
            "weather_position" => "Weather block position",
            "dock_position" => "Dock position",
            "dock_background_color" => "Dock background color",
            "bottom" => "Bottom",
            "top" => "Top",
            "left" => "Left",
            "right" => "Right",
            "settings_save_error" => "Save error",
            "configuration_backup" => "Configuration backup",
            "export_config" => "Export…",
            "import_config" => "Import…",
            "export_config_title" => "Export configuration",
            "import_config_title" => "Import configuration",
            "export_config_done" => "Configuration exported to",
            "export_config_error" => "Unable to export the configuration",
            "import_config_done" => "Configuration imported",
            "import_config_invalid" => "This file is not a valid Dock Windows configuration.",
            "invalid_drop" => "Some dropped items are not accessible files or folders.",
            "launch_error" => "Unable to open",
            "startup_error" => "Unable to change Windows startup",
            "shutdown_error" => "Unable to shut down the computer",
            "search_hint" => "Search",
            _ => key,
        },
        AppLanguage::Spanish => match key {
            "settings" => "Configuración",
            "close" => "Cerrar",
            "menu_section_application" => "Aplicación",
            "menu_section_organize" => "Organización",
            "menu_section_dock" => "Dock",
            "settings_section_appearance" => "Apariencia",
            "settings_section_content" => "Contenido del dock",
            "settings_section_position" => "Posición y colores",
            "settings_section_system" => "Sistema",
            "settings_section_backup" => "Copia de seguridad",
            "setup_title" => "Te damos la bienvenida a Dock Windows",
            "setup_subtitle" => {
                "Elige el idioma del dock y los accesos directos que quieres importar."
            }
            "setup_language" => "Idioma de la aplicación",
            "setup_shortcuts" => "Accesos directos de Windows",
            "setup_taskbar" => "Barra de tareas",
            "setup_desktop" => "Escritorio",
            "setup_start_menu" => "Menú Inicio",
            "setup_summary_one" => "Se añadirá {count} acceso directo al dock.",
            "setup_summary_many" => "Se añadirán {count} accesos directos al dock.",
            "setup_summary_none" => "No se importará ningún acceso directo por ahora.",
            "setup_hint" => "Puedes cambiar estas opciones en cualquier momento en Configuración.",
            "setup_start" => "Empezar",
            "setup_skip" => "Omitir",
            "setup_group_default" => "General",
            "remove_shortcut" => "Quitar del dock",
            "add_current_group" => "Añadir al grupo actual",
            "add_to_group" => "Añadir a un grupo",
            "move_to_group" => "Mover a un grupo",
            "drop_into_group" => "Soltar en un grupo",
            "open_windows" => "Ventanas abiertas",
            "close_window" => "Cerrar una ventana",
            "close_this_window" => "Cerrar esta ventana",
            "close_all_windows" => "Cerrar todas las ventanas de la aplicación",
            "close_denied" => "Windows no aceptó la solicitud de cierre.",
            "close_unsupported" => "El cierre nativo solo es compatible con Windows.",
            "power_off" => "Apagar el equipo",
            "weather_at" => "Tiempo en",
            "weather_loading_at" => "Cargando el tiempo en",
            "weather_request_error" => "No se puede iniciar la actualización del tiempo",
            "local_time" => "Hora local",
            "quit" => "Salir",
            "no_apps_found" => "No se encontraron aplicaciones.",
            "search_error" => "No se pueden buscar aplicaciones",
            "preview_unavailable" => "Vista previa no disponible",
            "shutdown_title" => "Confirmar apagado",
            "shutdown_question" => "¿Seguro que quieres apagar el equipo?",
            "cancel" => "Cancelar",
            "shutdown_now" => "Apagar ahora",
            "groups" => "Grupos",
            "new_group" => "Nuevo grupo",
            "delete_group" => "Eliminar",
            "rename_group" => "Renombrar",
            "group_name" => "Nombre del grupo",
            "create_group_action" => "Crear",
            "rename_group_action" => "Renombrar",
            "group_name_empty" => "El nombre del grupo no puede estar vacío.",
            "group_name_exists" => "Ya existe un grupo con este nombre.",
            "hide_application" => "Ocultar esta aplicación",
            "hidden_applications" => "Aplicaciones ocultas",
            "show_application" => "Mostrar",
            "show_all_applications" => "Mostrar todo",
            "icon_size" => "Tamaño de los iconos",
            "vertical_spacing" => "Espacio vertical alrededor de los iconos",
            "background_opacity" => "Opacidad del fondo",
            "icon_opacity" => "Opacidad de los iconos",
            "show_clock" => "Mostrar la hora",
            "show_weather" => "Mostrar el tiempo",
            "show_shutdown_button" => "Mostrar el botón de apagado",
            "show_wifi" => "Mostrar Wi-Fi",
            "show_volume" => "Mostrar sonido",
            "show_search" => "Mostrar el campo de búsqueda",
            "wifi_error" => "No se pueden abrir las redes Wi-Fi",
            "volume_error" => "No se puede ajustar el volumen",
            "volume" => "Volumen",
            "always_on_top" => "Mantener el dock siempre visible",
            "startup" => "Iniciar con Windows",
            "language" => "Idioma",
            "weather_city" => "Ciudad del tiempo",
            "apply" => "Aplicar",
            "shutdown_position" => "Posición del botón de apagado",
            "weather_position" => "Posición del bloque del tiempo",
            "dock_position" => "Posición del dock",
            "dock_background_color" => "Color de fondo del dock",
            "bottom" => "Abajo",
            "top" => "Arriba",
            "left" => "Izquierda",
            "right" => "Derecha",
            "settings_save_error" => "Error al guardar",
            "configuration_backup" => "Copia de seguridad de la configuración",
            "export_config" => "Exportar…",
            "import_config" => "Importar…",
            "export_config_title" => "Exportar la configuración",
            "import_config_title" => "Importar la configuración",
            "export_config_done" => "Configuración exportada a",
            "export_config_error" => "No se puede exportar la configuración",
            "import_config_done" => "Configuración importada",
            "import_config_invalid" => {
                "Este archivo no es una configuración válida de Dock Windows."
            }
            "invalid_drop" => "Algunos elementos soltados no son archivos o carpetas accesibles.",
            "launch_error" => "No se puede abrir",
            "startup_error" => "No se puede cambiar el inicio de Windows",
            "shutdown_error" => "No se puede apagar el equipo",
            "search_hint" => "Buscar",
            _ => key,
        },
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct DockConfig {
    #[serde(default = "default_groups")]
    groups: Vec<ShortcutGroup>,
    #[serde(default)]
    selected_group: usize,
    #[serde(default = "default_icon_size")]
    icon_size: f32,
    #[serde(default = "default_vertical_padding")]
    vertical_padding: u8,
    #[serde(default = "default_background_opacity", alias = "opacity")]
    background_opacity: u8,
    #[serde(default = "default_icon_opacity")]
    icon_opacity: u8,
    #[serde(default)]
    show_clock: bool,
    #[serde(default)]
    show_weather: bool,
    #[serde(default)]
    show_wifi: bool,
    #[serde(default)]
    show_volume: bool,
    #[serde(default)]
    show_shutdown_button: bool,
    #[serde(default)]
    show_search: bool,
    #[serde(default = "default_dock_side")]
    shutdown_side: DockSide,
    #[serde(default = "default_dock_side")]
    weather_side: DockSide,
    #[serde(default)]
    dock_position: DockPosition,
    #[serde(default = "default_dock_background_color")]
    background_color: [u8; 4],
    #[serde(default)]
    language: AppLanguage,
    #[serde(default = "default_always_on_top")]
    always_on_top: bool,
    #[serde(default = "default_weather_city")]
    weather_city: String,
    #[serde(default)]
    hidden_apps: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ShortcutGroup {
    name: String,
    #[serde(default)]
    shortcuts: Vec<Shortcut>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Shortcut {
    path: PathBuf,
}

impl Default for DockConfig {
    fn default() -> Self {
        Self {
            groups: default_groups(),
            selected_group: 0,
            icon_size: DEFAULT_ICON_SIZE,
            vertical_padding: DEFAULT_VERTICAL_PADDING,
            background_opacity: DEFAULT_OPACITY,
            icon_opacity: DEFAULT_ICON_OPACITY,
            show_clock: false,
            show_weather: false,
            show_wifi: false,
            show_volume: false,
            show_shutdown_button: false,
            show_search: false,
            shutdown_side: DockSide::Right,
            weather_side: DockSide::Right,
            dock_position: DockPosition::Bottom,
            background_color: default_dock_background_color(),
            language: AppLanguage::French,
            always_on_top: default_always_on_top(),
            weather_city: default_weather_city(),
            hidden_apps: Vec::new(),
        }
    }
}

fn default_icon_size() -> f32 {
    DEFAULT_ICON_SIZE
}

fn default_vertical_padding() -> u8 {
    DEFAULT_VERTICAL_PADDING
}

fn default_background_opacity() -> u8 {
    DEFAULT_OPACITY
}

fn default_icon_opacity() -> u8 {
    DEFAULT_ICON_OPACITY
}

fn default_weather_city() -> String {
    DEFAULT_WEATHER_CITY.to_owned()
}

fn default_always_on_top() -> bool {
    true
}

fn default_dock_side() -> DockSide {
    DockSide::Right
}

fn default_dock_background_color() -> [u8; 4] {
    [24, 26, 32, 175]
}

fn default_groups() -> Vec<ShortcutGroup> {
    vec![ShortcutGroup {
        name: "Général".to_owned(),
        shortcuts: Vec::new(),
    }]
}

impl DockConfig {
    /// Corrige les valeurs aberrantes et supprime les raccourcis dupliques entre
    /// groupes. Retourne `true` si des doublons ont ete retires.
    fn normalize(&mut self) -> bool {
        if self.groups.is_empty() {
            self.groups = default_groups();
        }
        self.selected_group = self.selected_group.min(self.groups.len() - 1);
        if !self.icon_size.is_finite() {
            self.icon_size = DEFAULT_ICON_SIZE;
        } else {
            self.icon_size = self.icon_size.clamp(MIN_ICON_SIZE, MAX_ICON_SIZE);
        }
        self.vertical_padding = self
            .vertical_padding
            .clamp(MIN_VERTICAL_PADDING, MAX_VERTICAL_PADDING);
        self.background_opacity = self.background_opacity.clamp(MIN_OPACITY, MAX_OPACITY);
        self.icon_opacity = self.icon_opacity.clamp(MIN_OPACITY, MAX_OPACITY);
        self.hidden_apps = self
            .hidden_apps
            .drain(..)
            .map(|path| path.replace('\\', "/").to_lowercase())
            .collect();
        self.hidden_apps.sort();
        self.hidden_apps.dedup();
        let mut seen = HashSet::new();
        let mut removed_duplicates = false;
        for group in &mut self.groups {
            let before = group.shortcuts.len();
            group
                .shortcuts
                .retain(|shortcut| seen.insert(path_key(&shortcut.path)));
            removed_duplicates |= group.shortcuts.len() != before;
        }
        removed_duplicates
    }

    fn is_app_hidden(&self, path: &Path) -> bool {
        let key = path_key(path);
        self.hidden_apps
            .iter()
            .any(|hidden| hidden.eq_ignore_ascii_case(&key))
    }

    fn hide_app(&mut self, path: &Path) -> bool {
        let key = path_key(path);
        if self
            .hidden_apps
            .iter()
            .any(|hidden| hidden.eq_ignore_ascii_case(&key))
        {
            return false;
        }
        self.hidden_apps.push(key);
        true
    }

    fn show_app(&mut self, key: &str) -> bool {
        let key = key.replace('\\', "/").to_lowercase();
        let before = self.hidden_apps.len();
        self.hidden_apps
            .retain(|hidden| !hidden.eq_ignore_ascii_case(&key));
        self.hidden_apps.len() != before
    }

    fn show_all_apps(&mut self) -> bool {
        if self.hidden_apps.is_empty() {
            return false;
        }
        self.hidden_apps.clear();
        true
    }

    #[cfg(test)]
    fn add_shortcut(&mut self, path: PathBuf) -> bool {
        self.add_shortcut_to_group(self.selected_group, path)
    }

    /// Cles de tous les raccourcis epingles, tous groupes confondus : une
    /// application deja presente dans un groupe ne doit pas apparaitre une
    /// seconde fois parmi les applications actives.
    fn pinned_keys(&self) -> Vec<String> {
        self.groups
            .iter()
            .flat_map(|group| group.shortcuts.iter())
            .map(|shortcut| path_key(&shortcut.path))
            .collect()
    }

    fn add_shortcut_to_group(&mut self, group_index: usize, path: PathBuf) -> bool {
        if group_index >= self.groups.len() {
            return false;
        }
        let path_text = path_key(&path);
        // Un raccourci ne peut etre epingle que dans un seul groupe : deux
        // icones identiques n'ont pas de sens dans le dock.
        if self.contains_shortcut(&path_text) {
            return false;
        }
        self.groups[group_index].shortcuts.push(Shortcut { path });
        true
    }

    /// Indique si un raccourci vers ce chemin est deja epingle, tous groupes
    /// confondus.
    fn contains_shortcut(&self, path_text: &str) -> bool {
        self.groups
            .iter()
            .flat_map(|group| group.shortcuts.iter())
            .any(|item| path_key(&item.path) == path_text)
    }

    /// Deplace un raccourci vers la fin d'un autre groupe. Le raccourci n'est
    /// pas duplique si le groupe cible le contient deja.
    fn move_shortcut(&mut self, from_group: usize, from_index: usize, to_group: usize) -> bool {
        if from_group == to_group {
            return false;
        }
        self.move_shortcut_to_position(from_group, from_index, to_group, usize::MAX)
    }

    /// Deplace un raccourci vers une position precise d'un groupe. Une position
    /// au-dela de la fin du groupe (comme `usize::MAX`) ajoute le raccourci en
    /// dernier. Le raccourci n'est pas duplique si le groupe cible le contient
    /// deja.
    fn move_shortcut_to_position(
        &mut self,
        from_group: usize,
        from_index: usize,
        to_group: usize,
        to_index: usize,
    ) -> bool {
        if to_group >= self.groups.len() {
            return false;
        }
        let Some(shortcut) = self
            .groups
            .get(from_group)
            .and_then(|group| group.shortcuts.get(from_index))
            .cloned()
        else {
            return false;
        };
        let path_text = path_key(&shortcut.path);
        self.groups[from_group].shortcuts.remove(from_index);
        // Le raccourci peut deja etre epingle ailleurs (ancienne configuration
        // avec doublons) : le retirer de son groupe suffit alors.
        let already_present = self.groups.iter().enumerate().any(|(index, group)| {
            index != from_group
                && group
                    .shortcuts
                    .iter()
                    .any(|item| path_key(&item.path) == path_text)
        });
        if already_present {
            return true;
        }
        // Retirer le raccourci decale d'un cran les elements suivants : la cible
        // doit etre corrigee pour retrouver la place visee sous le curseur.
        let mut target_index = to_index;
        if from_group == to_group && from_index < target_index {
            target_index -= 1;
        }
        let target_index = target_index.min(self.groups[to_group].shortcuts.len());
        self.groups[to_group]
            .shortcuts
            .insert(target_index, shortcut);
        true
    }

    fn add_group(&mut self, name: &str) -> Result<(), &'static str> {
        let name = name.trim();
        if name.is_empty() {
            return Err("group_name_empty");
        }
        if self
            .groups
            .iter()
            .any(|group| group.name.eq_ignore_ascii_case(name))
        {
            return Err("group_name_exists");
        }
        self.groups.push(ShortcutGroup {
            name: name.to_owned(),
            shortcuts: Vec::new(),
        });
        self.selected_group = self.groups.len() - 1;
        Ok(())
    }

    fn rename_group(&mut self, index: usize, name: &str) -> Result<(), &'static str> {
        let name = name.trim();
        if name.is_empty() {
            return Err("group_name_empty");
        }
        if self
            .groups
            .iter()
            .enumerate()
            .any(|(position, group)| position != index && group.name.eq_ignore_ascii_case(name))
        {
            return Err("group_name_exists");
        }
        let Some(group) = self.groups.get_mut(index) else {
            return Err("group_name_empty");
        };
        group.name = name.to_owned();
        Ok(())
    }

    /// Supprime un groupe precis et deplace ses raccourcis dans le premier
    /// groupe restant.
    fn remove_group(&mut self, index: usize) -> bool {
        if self.groups.len() <= 1 || index >= self.groups.len() {
            return false;
        }
        let removed = self.groups.remove(index);
        self.groups[0].shortcuts.extend(removed.shortcuts);
        if self.selected_group >= self.groups.len() {
            self.selected_group = self.groups.len() - 1;
        }
        true
    }
}

struct DockApp {
    config: DockConfig,
    config_path: PathBuf,
    running_apps: Vec<RunningApp>,
    search_apps: Vec<SearchApp>,
    search_error: Option<String>,
    search_query: String,
    icon_cache: HashMap<String, Option<TextureHandle>>,
    last_active_refresh: Option<Instant>,
    last_window_width: Option<f32>,
    last_window_height: Option<f32>,
    last_window_position: Option<egui::Pos2>,
    dock_layout: DockLayout,
    dragging_item: Option<DragItem>,
    group_drop_rects: Vec<Option<egui::Rect>>,
    last_viewport_resizable: Option<bool>,
    last_dock_position: DockPosition,
    context_menu_open: bool,
    settings_popup_anchor: Option<egui::Rect>,
    settings_popup_position: Option<egui::Pos2>,
    settings_popup_has_focus: Arc<AtomicBool>,
    settings_popup_dismiss: Arc<AtomicBool>,
    settings_changes_receiver: Receiver<SettingsChanges>,
    settings_changes_sender: Sender<SettingsChanges>,
    pending_settings: SettingsChanges,
    settings_dirty: bool,
    settings_persist_pending: bool,
    last_settings_persist: Option<Instant>,
    fallback_context_menu_position: Option<egui::Pos2>,
    window_preview: Option<WindowPreviewGroup>,
    startup_enabled: bool,
    weather: Option<weather::CurrentWeather>,
    weather_error: Option<String>,
    weather_receiver: Receiver<(String, Result<weather::CurrentWeather, String>)>,
    weather_sender: Sender<(String, Result<weather::CurrentWeather, String>)>,
    weather_request_in_flight: bool,
    weather_request_city: Option<String>,
    weather_city_draft: String,
    last_weather_request: Option<Instant>,
    shutdown_confirmation_open: bool,
    system_volume: Option<f32>,
    system_volume_error: Option<String>,
    last_volume_refresh: Option<Instant>,
    setup: Option<SetupState>,
    setup_window_ready: bool,
}

/// Assistant affiche au tout premier demarrage : langue du dock et import des
/// raccourcis Windows. La configuration n'est ecrite qu'a la validation, donc
/// l'assistant reapparait si l'application est fermee avant la fin.
struct SetupState {
    language: AppLanguage,
    taskbar: Vec<PathBuf>,
    desktop: Vec<PathBuf>,
    start_menu: Vec<PathBuf>,
    import_taskbar: bool,
    import_desktop: bool,
    import_start_menu: bool,
}

impl SetupState {
    fn new(language: AppLanguage, search_apps: &[SearchApp]) -> Self {
        let taskbar = taskbar_shortcuts();
        let desktop = desktop_shortcuts();
        let start_menu: Vec<PathBuf> = search_apps
            .iter()
            .map(|app| app.path.clone())
            .filter(|path| !is_dock_shortcut(path))
            .collect();
        Self {
            language,
            import_taskbar: !taskbar.is_empty(),
            import_desktop: !desktop.is_empty(),
            import_start_menu: false,
            taskbar,
            desktop,
            start_menu,
        }
    }

    fn selected_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let mut seen = HashSet::new();
        for (enabled, source) in [
            (self.import_taskbar, &self.taskbar),
            (self.import_desktop, &self.desktop),
            (self.import_start_menu, &self.start_menu),
        ] {
            if !enabled {
                continue;
            }
            for path in source {
                if seen.insert(path_key(path)) {
                    paths.push(path.clone());
                }
            }
        }
        paths
    }

    fn total(&self) -> usize {
        self.selected_paths().len()
    }
}

/// Taille du dock calculee une seule fois par image, puis utilisee a la fois
/// pour dimensionner la fenetre et pour dessiner le contenu.
#[derive(Clone, Copy, Debug, Default)]
struct DockLayout {
    area_width: f32,
    area_height: f32,
    wrap: bool,
}

/// Element en cours de deplacement vers un autre groupe.
#[derive(Clone, Debug)]
enum DragItem {
    /// Raccourci epingle : deplace vers le groupe cible.
    Shortcut { group: usize, index: usize },
    /// Application en cours d'execution : epingle dans le groupe cible.
    RunningApp { path: PathBuf },
}

#[derive(Clone, Debug)]
struct SearchApp {
    name: String,
    path: PathBuf,
}

struct RunningApp {
    path: PathBuf,
    windows: Vec<RunningWindow>,
    icon: Option<TextureHandle>,
}

#[derive(Clone)]
struct RunningWindow {
    title: String,
    window: isize,
}

struct WindowPreviewGroup {
    app_key: String,
    app_name: String,
    windows: Vec<RunningWindow>,
    thumbnails: Vec<Option<window_thumbnails::Thumbnail>>,
    thumbnail_errors: Vec<Option<String>>,
}

impl DockApp {
    fn load() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let config_path = config_path()?;
        let first_run = !config_path.exists() && !config_backup_path(&config_path).exists();
        let mut config = load_config_with_backup(&config_path)?;
        if config.normalize() {
            // Des doublons existaient (ancienne configuration) : on les retire
            // aussi du fichier.
            let _ = write_config(&config_path, &config);
        }
        let startup_enabled = startup::is_enabled()?;
        let (search_apps, search_error) = match start_menu_apps() {
            Ok(apps) => (apps, None),
            Err(error) => (Vec::new(), Some(error.to_string())),
        };
        let weather_city_draft = config.weather_city.clone();
        let initial_position = config.dock_position;
        let setup = first_run.then(|| SetupState::new(system_language(), &search_apps));
        let (weather_sender, weather_receiver) = mpsc::channel();
        let (settings_changes_sender, settings_changes_receiver) = mpsc::channel();

        Ok(Self {
            config,
            config_path,
            running_apps: Vec::new(),
            search_apps,
            search_error,
            search_query: String::new(),
            icon_cache: HashMap::new(),
            last_active_refresh: None,
            last_window_width: None,
            last_window_height: None,
            last_window_position: None,
            dock_layout: DockLayout::default(),
            dragging_item: None,
            group_drop_rects: Vec::new(),
            last_viewport_resizable: Some(false),
            last_dock_position: initial_position,
            context_menu_open: false,
            settings_popup_anchor: None,
            settings_popup_position: None,
            settings_popup_has_focus: Arc::new(AtomicBool::new(false)),
            settings_popup_dismiss: Arc::new(AtomicBool::new(false)),
            settings_changes_receiver,
            settings_changes_sender,
            pending_settings: SettingsChanges::default(),
            settings_dirty: false,
            settings_persist_pending: false,
            last_settings_persist: None,
            fallback_context_menu_position: None,
            window_preview: None,
            startup_enabled,
            weather: None,
            weather_error: None,
            weather_receiver,
            weather_sender,
            weather_request_in_flight: false,
            weather_request_city: None,
            weather_city_draft,
            last_weather_request: None,
            shutdown_confirmation_open: false,
            system_volume: None,
            system_volume_error: None,
            last_volume_refresh: None,
            setup,
            setup_window_ready: false,
        })
    }

    fn persist(&mut self) -> bool {
        match write_config(&self.config_path, &self.config) {
            Ok(()) => true,
            Err(error) => {
                show_error(&format!(
                    "{}: {error}",
                    tr(self.config.language, "settings_save_error")
                ));
                false
            }
        }
    }

    /// Premier demarrage : la fenetre du dock devient un panneau centre qui
    /// laisse choisir la langue et les raccourcis Windows a importer.
    fn show_setup(&mut self, ctx: &egui::Context) {
        if !self.setup_window_ready {
            let work_area = monitor_work_area(ctx);
            let size = egui::vec2(SETUP_WINDOW_WIDTH, SETUP_WINDOW_HEIGHT)
                .min((work_area.size() - egui::vec2(24.0, 24.0)).max(egui::Vec2::splat(320.0)));
            let position = work_area.center() - size * 0.5;
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(position));
            self.setup_window_ready = true;
            ctx.request_repaint();
        }

        let Some(setup) = self.setup.as_mut() else {
            return;
        };
        let language = setup.language;
        let mut finished = false;
        let mut skipped = false;

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(20, 22, 29, 252))
                    .corner_radius(egui::CornerRadius::same(22))
                    .stroke(egui::Stroke::new(1.0, Color32::from_white_alpha(30)))
                    .inner_margin(egui::Margin::same(26)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    app_mark(ui, 46.0);
                    ui.add_space(10.0);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(tr(language, "setup_title"))
                                .size(21.0)
                                .strong(),
                        );
                        ui.label(
                            egui::RichText::new(tr(language, "setup_subtitle"))
                                .size(12.5)
                                .color(menu_muted_color()),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        ui.label(
                            egui::RichText::new(format!("v{}", display_version()))
                                .size(11.0)
                                .color(menu_muted_color()),
                        );
                    });
                });

                settings_section_label(ui, tr(language, "setup_language"));
                ui.horizontal_wrapped(|ui| {
                    for choice in [
                        AppLanguage::French,
                        AppLanguage::English,
                        AppLanguage::Spanish,
                    ] {
                        let selected = setup.language == choice;
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(choice.native_name()).strong(),
                                )
                                .selected(selected)
                                .corner_radius(egui::CornerRadius::same(9))
                                .min_size(egui::vec2(112.0, 34.0)),
                            )
                            .clicked()
                        {
                            setup.language = choice;
                        }
                    }
                });

                settings_section_label(ui, tr(language, "setup_shortcuts"));
                setup_source_row(
                    ui,
                    &mut setup.import_taskbar,
                    MENU_ICON_PIN,
                    tr(language, "setup_taskbar"),
                    setup.taskbar.len(),
                );
                setup_source_row(
                    ui,
                    &mut setup.import_desktop,
                    MENU_ICON_WINDOWS,
                    tr(language, "setup_desktop"),
                    setup.desktop.len(),
                );
                setup_source_row(
                    ui,
                    &mut setup.import_start_menu,
                    MENU_ICON_MENU,
                    tr(language, "setup_start_menu"),
                    setup.start_menu.len(),
                );

                ui.add_space(10.0);
                let total = setup.total();
                ui.label(
                    egui::RichText::new(setup_summary(setup.language, total)).color(if total > 0 {
                        menu_text_color()
                    } else {
                        menu_muted_color()
                    }),
                );
                ui.label(
                    egui::RichText::new(tr(setup.language, "setup_hint"))
                        .size(11.0)
                        .color(menu_muted_color()),
                );

                let remaining = ui.available_height();
                if remaining > 52.0 {
                    ui.add_space(remaining - 44.0);
                } else {
                    ui.add_space(12.0);
                }
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if primary_button(ui, tr(setup.language, "setup_start")).clicked() {
                            finished = true;
                        }
                        ui.add_space(8.0);
                        if ui.link(tr(setup.language, "setup_skip")).clicked() {
                            skipped = true;
                        }
                    });
                });
            });

        if ctx.input(|input| input.key_pressed(egui::Key::Enter)) {
            finished = true;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            skipped = true;
        }
        if finished || skipped {
            self.finish_setup(ctx, skipped);
        }
    }

    fn finish_setup(&mut self, ctx: &egui::Context, skipped: bool) {
        let Some(setup) = self.setup.take() else {
            return;
        };
        self.config.language = setup.language;
        self.config.groups = setup_groups(
            setup.language,
            &setup.taskbar,
            &setup.desktop,
            &setup.start_menu,
            !skipped && setup.import_taskbar,
            !skipped && setup.import_desktop,
            !skipped && setup.import_start_menu,
        );
        self.config.selected_group = 0;
        self.config.normalize();
        self.persist();
        self.setup_window_ready = false;
        self.last_window_width = None;
        self.last_window_height = None;
        self.last_window_position = None;
        self.last_dock_position = self.config.dock_position;
        self.weather_city_draft = self.config.weather_city.clone();
        ctx.request_repaint();
    }

    /// Enregistre la configuration dans un fichier choisi par l'utilisateur.
    fn export_config(&mut self) {
        let language = self.config.language;
        match file_dialog::pick_json_file(
            true,
            tr(language, "export_config_title"),
            "dock-windows-config.json",
        ) {
            Some(path) => match write_config(&path, &self.config) {
                Ok(()) => show_info(&format!(
                    "{}:\n{}",
                    tr(language, "export_config_done"),
                    path.display()
                )),
                Err(error) => {
                    show_error(&format!("{}: {error}", tr(language, "export_config_error")))
                }
            },
            None => {}
        }
    }

    /// Remplace la configuration par un fichier choisi par l'utilisateur.
    fn import_config(&mut self, ctx: &egui::Context) {
        let language = self.config.language;
        match file_dialog::pick_json_file(false, tr(language, "import_config_title"), "") {
            Some(path) => {
                let imported = fs::read(&path)
                    .ok()
                    .and_then(|contents| parse_config(&contents));
                let Some(mut imported) = imported else {
                    show_error(tr(language, "import_config_invalid"));
                    return;
                };
                imported.normalize();
                // Garde l'ancienne configuration en secours avant de la
                // remplacer.
                let _ = write_config(&self.config_path, &self.config);
                self.config = imported;
                self.weather_city_draft = self.config.weather_city.clone();
                let window_level = if self.config.always_on_top {
                    egui::WindowLevel::AlwaysOnTop
                } else {
                    egui::WindowLevel::Normal
                };
                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(window_level));
                self.refresh_active_apps(ctx);
                let _ = self.persist();
                ctx.request_repaint();
                let shortcut_count: usize = self
                    .config
                    .groups
                    .iter()
                    .map(|group| group.shortcuts.len())
                    .sum();
                show_info(&format!(
                    "{}: {} / {}",
                    tr(self.config.language, "import_config_done"),
                    self.config.groups.len(),
                    shortcut_count
                ));
            }
            None => {}
        }
    }

    fn import_dropped_files(&mut self, files: Vec<egui::DroppedFile>, group_index: usize) {
        let mut changed = false;
        let mut invalid = 0;

        for file in files {
            match file.path {
                Some(path) if path.exists() => {
                    changed |= self.config.add_shortcut_to_group(group_index, path);
                }
                _ => invalid += 1,
            }
        }

        if changed {
            let _ = self.persist();
        }
        if invalid > 0 {
            show_error(tr(self.config.language, "invalid_drop"));
        }
    }

    fn launch(&mut self, path: &Path) {
        if let Err(error) = open::that(path) {
            show_error(&format!(
                "{} {}: {error}",
                tr(self.config.language, "launch_error"),
                display_name(path)
            ));
        }
    }

    fn refresh_active_apps(&mut self, ctx: &egui::Context) {
        let apps = active_apps::list();
        let active_windows: HashSet<isize> = apps.iter().map(|app| app.window).collect();
        if self.window_preview.as_ref().is_some_and(|preview| {
            preview
                .windows
                .iter()
                .any(|window| !active_windows.contains(&window.window))
        }) {
            self.window_preview = None;
        }
        let mut retained_icons: HashSet<String> = apps.iter().map(|app| app.path_key()).collect();
        retained_icons.extend(
            self.config
                .groups
                .iter()
                .flat_map(|group| group.shortcuts.iter())
                .map(|shortcut| path_key(&shortcut.path)),
        );
        self.icon_cache
            .retain(|path, _| retained_icons.contains(path));

        let mut running_apps = group_running_apps(apps);
        running_apps.retain(|app| !self.config.is_app_hidden(&app.path));
        for app in &mut running_apps {
            app.icon = self.icon_for_path(ctx, &app.path);
        }
        self.running_apps = running_apps;
    }

    fn refresh_weather(&mut self) {
        while let Ok((city, result)) = self.weather_receiver.try_recv() {
            if self.weather_request_city.as_deref() == Some(city.as_str()) {
                self.weather_request_in_flight = false;
                self.weather_request_city = None;
            }
            if city != self.config.weather_city {
                continue;
            }
            match result {
                Ok(weather) => {
                    self.weather = Some(weather);
                    self.weather_error = None;
                }
                Err(error) => {
                    self.weather_error = Some(error);
                }
            }
        }

        if !self.config.show_weather
            || self.weather_request_in_flight
            || self.last_weather_request.is_some_and(|last| {
                let interval = if self.weather_error.is_some() {
                    WEATHER_RETRY_INTERVAL
                } else {
                    WEATHER_REFRESH_INTERVAL
                };
                last.elapsed() < interval
            })
        {
            return;
        }

        let city = self.config.weather_city.clone();
        let language = self.config.language;
        let sender = self.weather_sender.clone();
        self.weather_request_in_flight = true;
        self.weather_request_city = Some(city.clone());
        self.last_weather_request = Some(Instant::now());
        if let Err(error) = thread::Builder::new()
            .name("dock-weather".to_owned())
            .spawn(move || {
                let result = weather::fetch_current(&city, language.code());
                let _ = sender.send((city, result));
            })
        {
            self.weather_request_in_flight = false;
            self.weather_request_city = None;
            self.weather_error = Some(format!(
                "{}: {error}",
                tr(language, "weather_request_error")
            ));
        }
    }

    /// Applique tout de suite les reglages qui influencent la taille ou le
    /// rendu du dock, avant le calcul de la fenetre. Le tour de boucle qui
    /// dessine le dock les reutilise ensuite pour l'enregistrement.
    fn apply_live_settings(&mut self) {
        let mut changed = false;
        if let Some(icon_size) = self.pending_settings.icon_size {
            let icon_size = icon_size.clamp(MIN_ICON_SIZE, MAX_ICON_SIZE);
            if self.config.icon_size != icon_size {
                self.config.icon_size = icon_size;
                changed = true;
            }
        }
        if let Some(vertical_padding) = self.pending_settings.vertical_padding {
            let vertical_padding =
                vertical_padding.clamp(MIN_VERTICAL_PADDING, MAX_VERTICAL_PADDING);
            if self.config.vertical_padding != vertical_padding {
                self.config.vertical_padding = vertical_padding;
                changed = true;
            }
        }
        if let Some(background_opacity) = self.pending_settings.background_opacity {
            let background_opacity = background_opacity.clamp(MIN_OPACITY, MAX_OPACITY);
            if self.config.background_opacity != background_opacity {
                self.config.background_opacity = background_opacity;
                changed = true;
            }
        }
        if let Some(icon_opacity) = self.pending_settings.icon_opacity {
            let icon_opacity = icon_opacity.clamp(MIN_OPACITY, MAX_OPACITY);
            if self.config.icon_opacity != icon_opacity {
                self.config.icon_opacity = icon_opacity;
                changed = true;
            }
        }
        for (value, target) in [
            (
                self.pending_settings.show_clock,
                &mut self.config.show_clock,
            ),
            (
                self.pending_settings.show_weather,
                &mut self.config.show_weather,
            ),
            (self.pending_settings.show_wifi, &mut self.config.show_wifi),
            (
                self.pending_settings.show_volume,
                &mut self.config.show_volume,
            ),
            (
                self.pending_settings.show_shutdown_button,
                &mut self.config.show_shutdown_button,
            ),
            (
                self.pending_settings.show_search,
                &mut self.config.show_search,
            ),
        ] {
            if let Some(value) = value {
                if *target != value {
                    *target = value;
                    changed = true;
                }
            }
        }
        if let Some(shutdown_side) = self.pending_settings.shutdown_side {
            if self.config.shutdown_side != shutdown_side {
                self.config.shutdown_side = shutdown_side;
                changed = true;
            }
        }
        if let Some(weather_side) = self.pending_settings.weather_side {
            if self.config.weather_side != weather_side {
                self.config.weather_side = weather_side;
                changed = true;
            }
        }
        if let Some(dock_position) = self.pending_settings.dock_position {
            if self.config.dock_position != dock_position {
                self.config.dock_position = dock_position;
                changed = true;
            }
        }
        if let Some(background_color) = self.pending_settings.background_color {
            if self.config.background_color != background_color {
                self.config.background_color = background_color;
                changed = true;
            }
        }
        if let Some(language) = self.pending_settings.language {
            if self.config.language != language {
                self.config.language = language;
                changed = true;
            }
        }
        if changed {
            self.settings_dirty = true;
        }
    }

    fn resize_to_content(&mut self, ctx: &egui::Context) {
        let dock_position_changed = self.last_dock_position != self.config.dock_position;
        let work_area = monitor_work_area(ctx);
        let settings_popup_open = self.context_menu_open
            || ctx.memory(|memory| memory.any_popup_open())
            || ctx.input(|input| input.pointer.secondary_down());
        let confirmation_open = self.shutdown_confirmation_open;
        let modal_open = confirmation_open;
        let resizable_changed = self.last_viewport_resizable != Some(settings_popup_open);
        if resizable_changed {
            ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(settings_popup_open));
            self.last_viewport_resizable = Some(settings_popup_open);
        }
        let vertical = self.config.dock_position.is_vertical();
        let shortcuts: usize = self
            .config
            .groups
            .iter()
            .map(|group| group.shortcuts.len())
            .sum();
        let item_spacing = ctx.style().spacing.item_spacing;
        let group_chrome_width = if vertical {
            0.0
        } else {
            group_separators_size(self.config.groups.len(), item_spacing.x)
        };
        let group_chrome_height = if vertical {
            group_separators_size(self.config.groups.len(), item_spacing.y)
        } else {
            0.0
        };
        let max_width = if vertical {
            VERTICAL_DOCK_WIDTH.min(available_dock_width(ctx))
        } else {
            available_dock_width(ctx)
        };
        let max_height = available_dock_height(ctx);
        let search_results_open = self.config.show_search && !self.search_query.trim().is_empty();
        let required_width = if vertical {
            VERTICAL_DOCK_WIDTH.min(available_dock_width(ctx))
        } else {
            dock_window_required_width_with_system_controls(
                shortcuts,
                self.visible_running_app_count(),
                ctx.style().spacing.item_spacing.x,
                self.config.icon_size,
                self.config.show_clock,
                self.config.show_weather,
                self.config.show_shutdown_button,
                self.config.show_search,
                system_control_count(self.config.show_wifi, self.config.show_volume),
            ) + group_chrome_width
        };
        let desired_width = if vertical {
            VERTICAL_DOCK_WIDTH.min(available_dock_width(ctx))
        } else {
            required_width.clamp(MIN_WINDOW_WIDTH, max_width)
        };
        let layout_width = if settings_popup_open && !vertical {
            self.last_window_width
                .unwrap_or(desired_width)
                .min(available_dock_width(ctx))
        } else {
            desired_width
        };
        let max_wrapped_item_width = (self.config.icon_size + ICON_CELL_PADDING)
            .max(if self.config.show_search {
                SEARCH_FIELD_WIDTH
            } else {
                0.0
            })
            .max(if self.config.show_weather {
                WEATHER_DISPLAY_WIDTH
            } else {
                0.0
            })
            .max(if self.config.show_clock {
                CLOCK_DISPLAY_WIDTH
            } else {
                0.0
            })
            .max(if self.config.show_wifi || self.config.show_volume {
                SYSTEM_CONTROL_SIZE
            } else {
                0.0
            });
        let wrapped_rows = if vertical {
            1
        } else {
            wrapped_layout_row_count(
                (required_width - FRAME_HORIZONTAL_MARGIN).max(1.0),
                (layout_width - FRAME_HORIZONTAL_MARGIN).max(1.0),
                max_wrapped_item_width,
            )
        };
        let desired_height = if vertical {
            dock_window_height_with_system_controls(
                shortcuts,
                self.visible_running_app_count(),
                ctx.style().spacing.item_spacing.y,
                self.config.icon_size,
                self.config.show_clock,
                self.config.show_weather,
                self.config.show_shutdown_button,
                self.config.show_search,
                system_control_count(self.config.show_wifi, self.config.show_volume),
                self.config.vertical_padding,
                max_height,
            ) + group_chrome_height
        } else {
            horizontal_dock_height_with_rows(
                shortcuts.saturating_add(self.visible_running_app_count()),
                self.config.icon_size,
                self.config.show_clock,
                self.config.show_weather,
                self.config.show_shutdown_button,
                self.config.show_search,
                system_control_count(self.config.show_wifi, self.config.show_volume),
                wrapped_rows,
                ctx.style().spacing.item_spacing.y,
                self.config.vertical_padding,
                max_height,
            )
        }
        .min(max_height);
        self.dock_layout = DockLayout {
            area_width: if vertical {
                desired_width
            } else {
                layout_width
            },
            area_height: desired_height,
            wrap: !vertical && required_width > layout_width + 1.0,
        };
        let preview_available_width = if vertical {
            (available_dock_width(ctx) - desired_width - POPUP_GAP).max(1.0)
        } else {
            available_dock_width(ctx)
        };
        let pointer_over_dock = cursor_position(ctx)
            .zip(ctx.input(|input| input.viewport().outer_rect))
            .is_some_and(|(pointer, window)| {
                let dock_rect = match self.config.dock_position {
                    DockPosition::Bottom => egui::Rect::from_min_max(
                        egui::pos2(window.left(), window.bottom() - desired_height),
                        window.max,
                    ),
                    DockPosition::Top => egui::Rect::from_min_max(
                        window.min,
                        egui::pos2(window.right(), window.top() + desired_height),
                    ),
                    DockPosition::Left => egui::Rect::from_min_max(
                        window.min,
                        egui::pos2(window.left() + desired_width, window.bottom()),
                    ),
                    DockPosition::Right => egui::Rect::from_min_max(
                        egui::pos2(window.right() - desired_width, window.top()),
                        window.max,
                    ),
                };
                dock_rect.contains(pointer)
            });
        let menu_space = settings_popup_open || pointer_over_dock;
        let preview_size = self
            .window_preview
            .as_ref()
            .map(|preview| window_preview_size(preview.windows.len(), preview_available_width));
        let preview_width = preview_size.map_or(0.0, |size| size.x);
        let preview_height = preview_size.map_or(0.0, |size| {
            size.y.min(available_preview_height(
                ctx,
                self.config.dock_position,
                desired_height,
            ))
        });
        let width = if menu_space && !vertical {
            self.last_window_width
                .unwrap_or(desired_width)
                .min(available_dock_width(ctx))
        } else {
            let popup_width = if vertical && menu_space {
                SETTINGS_MENU_EXTRA_HEIGHT
            } else if vertical && search_results_open {
                desired_width
            } else if vertical {
                preview_width
            } else {
                0.0
            };
            if vertical {
                (desired_width + popup_width).min(available_dock_width(ctx))
            } else {
                desired_width
                    .max(preview_width)
                    .min(available_dock_width(ctx))
            }
        };
        let width = if modal_open {
            width
                .max(CONFIRMATION_MIN_WIDTH)
                .min(available_dock_width(ctx))
        } else {
            width
        };
        let popup_height = (if menu_space {
            SETTINGS_MENU_EXTRA_HEIGHT
        } else {
            0.0
        })
        .max(if search_results_open {
            SEARCH_RESULTS_MAX_HEIGHT
        } else {
            0.0
        })
        .max(preview_height);
        let height = if vertical {
            desired_height.max(popup_height).max(if modal_open {
                CONFIRMATION_EXTRA_HEIGHT
            } else {
                0.0
            })
        } else {
            desired_height
                + if menu_space {
                    SETTINGS_MENU_EXTRA_HEIGHT
                } else {
                    0.0
                }
                + if search_results_open {
                    SEARCH_RESULTS_MAX_HEIGHT
                } else {
                    0.0
                }
                + if modal_open {
                    CONFIRMATION_EXTRA_HEIGHT
                } else {
                    0.0
                }
                + preview_height
        };
        let height = height.min(max_height);
        let width_changed = self
            .last_window_width
            .map_or(true, |current| (current - width).abs() > 1.0);
        let height_changed = self
            .last_window_height
            .map_or(true, |current| (current - height).abs() > 1.0);
        let [x, y] = dock_viewport_position(self.config.dock_position, width, height, work_area);
        let position = egui::pos2(x, y);
        let position_changed = self.last_window_position.map_or(true, |previous| {
            (previous.x - position.x).abs() > 1.0 || (previous.y - position.y).abs() > 1.0
        });
        if width_changed || height_changed {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(width, height)));
        }
        if position_changed {
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(position));
        }
        self.last_window_width = Some(width);
        self.last_window_height = Some(height);
        self.last_window_position = Some(position);
        if resizable_changed {
            // Le changement de style de fenetre peut faire perdre le premier
            // redimensionnement : on force un nouveau calcul a l'image suivante.
            self.last_window_width = None;
            self.last_window_height = None;
            ctx.request_repaint();
        }
        if dock_position_changed {
            self.last_dock_position = self.config.dock_position;
        }
    }

    /// Nombre d'applications actives a afficher : celles deja epinglees dans le
    /// groupe courant n'apparaissent pas une seconde fois.
    fn visible_running_app_count(&self) -> usize {
        count_visible_running_apps(&self.running_apps, &self.config.pinned_keys())
    }

    fn icon_for_path(&mut self, ctx: &egui::Context, path: &Path) -> Option<TextureHandle> {
        let key = path_key(path);
        if let Some(icon) = self.icon_cache.get(&key) {
            return icon.clone();
        }
        let icon = active_apps::load_icon(path).map(|image| {
            ctx.load_texture(
                format!("dock-icon-{key}"),
                image,
                egui::TextureOptions::LINEAR,
            )
        });
        self.icon_cache.insert(key, icon.clone());
        icon
    }

    fn show_dock(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        background_response: &egui::Response,
        vertical: bool,
        wrap_horizontal_layout: bool,
    ) {
        let mut changed = false;
        let language = self.config.language;
        let weather_city = self.config.weather_city.clone();
        let weather_city_draft = self.weather_city_draft.clone();
        let group_names: Vec<String> = self
            .config
            .groups
            .iter()
            .map(|group| group.name.clone())
            .collect();
        let hidden_apps = self.config.hidden_apps.clone();
        let group_shortcuts: Vec<Vec<Shortcut>> = self
            .config
            .groups
            .iter()
            .map(|group| group.shortcuts.clone())
            .collect();
        let current_time = self
            .config
            .show_clock
            .then(|| chrono::Local::now().format("%H:%M").to_string());
        let current_weather = self.weather.clone();
        let weather_error = self.weather_error.clone();
        let settings = SettingsState {
            icon_size: self.config.icon_size,
            vertical_padding: self.config.vertical_padding,
            background_opacity: self.config.background_opacity,
            icon_opacity: self.config.icon_opacity,
            show_clock: self.config.show_clock,
            show_weather: self.config.show_weather,
            show_wifi: self.config.show_wifi,
            show_volume: self.config.show_volume,
            show_shutdown_button: self.config.show_shutdown_button,
            show_search: self.config.show_search,
            shutdown_side: self.config.shutdown_side,
            weather_side: self.config.weather_side,
            dock_position: self.config.dock_position,
            background_color: self.config.background_color,
            startup_enabled: self.startup_enabled,
            always_on_top: self.config.always_on_top,
            weather_city: &weather_city,
            language,
        };
        let icon_opacity = settings.icon_opacity as f32 / 100.0;
        let group_icons: Vec<Vec<Option<TextureHandle>>> = group_shortcuts
            .iter()
            .map(|shortcuts| {
                shortcuts
                    .iter()
                    .map(|shortcut| self.icon_for_path(ctx, &shortcut.path))
                    .collect()
            })
            .collect();
        let pinned_keys = self.config.pinned_keys();
        let apps: Vec<(PathBuf, Vec<RunningWindow>, Option<TextureHandle>)> = self
            .running_apps
            .iter()
            .filter(|app| !pinned_keys.contains(&path_key(&app.path)))
            .map(|app| (app.path.clone(), app.windows.clone(), app.icon.clone()))
            .collect();
        let running_windows: HashMap<String, Vec<RunningWindow>> = self
            .running_apps
            .iter()
            .map(|app| (path_key(&app.path), app.windows.clone()))
            .collect();
        let left_power = settings.show_shutdown_button && settings.shutdown_side == DockSide::Left;
        let left_information = settings.show_weather && settings.weather_side == DockSide::Left;
        let right_power =
            settings.show_shutdown_button && settings.shutdown_side == DockSide::Right;
        let right_information = settings.show_clock
            || (settings.show_weather && settings.weather_side == DockSide::Right);
        let right_system_control_count =
            system_control_count(settings.show_wifi, settings.show_volume);
        let has_left_controls = left_power || left_information;
        let total_shortcuts: usize = group_shortcuts
            .iter()
            .map(|shortcuts| shortcuts.len())
            .sum();
        let has_center_content = settings.show_search
            || total_shortcuts > 0
            || !apps.is_empty()
            || !group_names.is_empty();
        let has_right_controls = right_power || right_information || right_system_control_count > 0;
        let mut volume = self.system_volume;
        let mut volume_change = None;
        let volume_error = self.system_volume_error.clone();
        let mut open_wifi_networks = false;
        let mut launch_path = None;
        let mut search_launch_path = None;
        let mut search_rect = None;
        let mut search_enter_pressed = false;
        let mut preview_request = None;
        let mut preview_request_anchor = None;
        let mut preview_anchor_rect = None;
        let mut preview_close_request = false;
        let mut remove_shortcut = None;
        let mut move_shortcut_request: Option<(usize, usize, usize)> = None;
        let mut activate_window = None;
        let mut pin_request = None;
        let mut close_window_request = None;
        let mut close_application_request = None;
        let mut hide_app: Option<PathBuf> = None;
        let mut show_app: Option<String> = None;
        let mut show_all_apps = false;
        let mut drop_slot: Option<(usize, usize)> = None;
        let pointer_released = ui.input(|input| input.pointer.any_released());
        let pointer_position = ui.input(|input| input.pointer.interact_pos());
        let dragging = self.dragging_item.is_some();
        let mut settings_changes = std::mem::take(&mut self.pending_settings);
        let mut context_menu_open = false;

        egui::ScrollArea::vertical()
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .scroll_offset(egui::Vec2::ZERO)
            .min_scrolled_width(0.0)
            .min_scrolled_height(0.0)
            .auto_shrink([true, true])
            .show(ui, |ui| {
                ui.with_layout(
                    if vertical {
                        egui::Layout::top_down(egui::Align::Center)
                    } else {
                        egui::Layout::left_to_right(egui::Align::Center)
                            .with_main_wrap(wrap_horizontal_layout)
                    },
                    |ui| {
                        if left_power {
                            let (clicked, menu_open) =
                                show_power_control(ui, settings, &mut settings_changes);
                            context_menu_open |= menu_open;
                            if clicked {
                                self.shutdown_confirmation_open = true;
                            }
                        }
                        if left_information {
                            if left_power {
                                dock_separator(
                                    ui,
                                    vertical,
                                    settings.icon_size + ICON_CELL_PADDING,
                                );
                            }
                            context_menu_open |= show_information_controls(
                                ui,
                                settings,
                                true,
                                false,
                                current_weather.as_ref(),
                                weather_error.as_deref(),
                                current_time.as_deref(),
                                &mut settings_changes,
                            );
                        }
                        if has_left_controls && has_center_content {
                            dock_separator(ui, vertical, settings.icon_size + ICON_CELL_PADDING);
                        }
                        if settings.show_search {
                            let (response, rect) = search_field(
                                ui,
                                &mut self.search_query,
                                language,
                                dock_background_color_with_opacity(
                                    settings.background_color,
                                    settings.background_opacity,
                                ),
                            );
                            search_rect = Some(rect);
                            context_menu_open |= add_settings_context_menu(
                                &response,
                                settings,
                                &mut settings_changes,
                            );
                            if response.has_focus()
                                && ui.input(|input| input.key_pressed(egui::Key::Enter))
                            {
                                search_enter_pressed = true;
                            }
                            if response.has_focus()
                                && ui.input(|input| input.key_pressed(egui::Key::Escape))
                            {
                                self.search_query.clear();
                            }
                            if total_shortcuts > 0 || !apps.is_empty() {
                                dock_separator(
                                    ui,
                                    vertical,
                                    settings.icon_size + ICON_CELL_PADDING,
                                );
                            }
                        }

                        let mut group_rects: Vec<Option<egui::Rect>> =
                            vec![None; group_names.len()];
                        let mut shortcut_rects: Vec<(usize, usize, egui::Rect)> = Vec::new();
                        let mut has_group_content = false;
                        for (group_index, (shortcuts, icons)) in
                            group_shortcuts.iter().zip(&group_icons).enumerate()
                        {
                            if shortcuts.is_empty() {
                                continue;
                            }
                            if has_group_content {
                                dock_separator(
                                    ui,
                                    vertical,
                                    settings.icon_size + ICON_CELL_PADDING,
                                );
                            }
                            has_group_content = true;
                            let mut group_rect: Option<egui::Rect> = None;

                            for (index, (shortcut, icon)) in shortcuts.iter().zip(icons).enumerate()
                            {
                                let key = path_key(&shortcut.path);
                                let running = running_windows.get(&key);
                                let response = ui
                                    .push_id(("pinned", &key), |ui| {
                                        icon_button(
                                            ui,
                                            icon.as_ref(),
                                            settings.icon_size,
                                            egui::Sense::click_and_drag(),
                                            icon_opacity,
                                        )
                                    })
                                    .inner;
                                let tooltip = running.map(|windows| {
                                    if windows.len() == 1 {
                                        windows[0].title.clone()
                                    } else {
                                        let word = match language {
                                            AppLanguage::French => "fenêtres de",
                                            AppLanguage::English => "windows for",
                                            AppLanguage::Spanish => "ventanas de",
                                        };
                                        format!(
                                            "{} {word} {}",
                                            windows.len(),
                                            display_name(&shortcut.path)
                                        )
                                    }
                                });
                                let response = match tooltip {
                                    Some(tooltip) => response.on_hover_text(tooltip),
                                    None => response,
                                };
                                group_rect = Some(match group_rect {
                                    Some(rect) => rect.union(response.rect),
                                    None => response.rect,
                                });
                                shortcut_rects.push((group_index, index, response.rect));
                                if running.is_some() {
                                    ui.painter().circle_filled(
                                        response.rect.center_bottom() - egui::vec2(0.0, 4.0),
                                        2.0,
                                        Color32::from_white_alpha(200),
                                    );
                                }
                                if self
                                    .window_preview
                                    .as_ref()
                                    .is_some_and(|preview| preview.app_key == key)
                                {
                                    preview_anchor_rect = Some(response.rect);
                                }
                                if response.drag_started() {
                                    self.dragging_item = Some(DragItem::Shortcut {
                                        group: group_index,
                                        index,
                                    });
                                }
                                if response.clicked() {
                                    match running {
                                        Some(windows) if windows.len() > 1 => {
                                            if self
                                                .window_preview
                                                .as_ref()
                                                .is_some_and(|preview| preview.app_key == key)
                                            {
                                                preview_close_request = true;
                                            } else {
                                                preview_request = Some((
                                                    key.clone(),
                                                    display_name(&shortcut.path),
                                                    windows.clone(),
                                                ));
                                                preview_request_anchor = Some(response.rect);
                                            }
                                        }
                                        Some(windows) if windows.len() == 1 => {
                                            activate_window =
                                                windows.first().map(|window| window.window);
                                            preview_close_request = true;
                                        }
                                        _ => launch_path = Some(shortcut.path.clone()),
                                    }
                                }
                                let _ = response.context_menu(|ui| {
                                    show_scrollable_menu(ui, "shortcut-context-menu", |ui| {
                                        menu_section(ui, tr(language, "menu_section_organize"));
                                        if menu_item(
                                            ui,
                                            MENU_ICON_REMOVE,
                                            tr(language, "remove_shortcut"),
                                        )
                                        .clicked()
                                        {
                                            remove_shortcut = Some((group_index, index));
                                            ui.close_menu();
                                        }
                                        if let Some(target) = move_shortcut_menu(
                                            ui,
                                            language,
                                            &group_names,
                                            group_index,
                                        ) {
                                            move_shortcut_request =
                                                Some((group_index, index, target));
                                        }
                                        menu_separator(ui);
                                        settings_menu(ui, settings, &mut settings_changes);
                                    });
                                });
                                context_menu_open |= response.context_menu_opened();
                            }
                            group_rects[group_index] = group_rect;
                        }
                        if dragging {
                            drop_slot = pointer_position.and_then(|pointer| {
                                group_rects
                                    .iter()
                                    .position(|rect| {
                                        rect.is_some_and(|rect| rect.contains(pointer))
                                    })
                                    .map(|target_group| {
                                        let index = match self.dragging_item {
                                            Some(DragItem::Shortcut { .. }) => {
                                                shortcut_insertion_index(
                                                    target_group,
                                                    pointer,
                                                    &shortcut_rects,
                                                    vertical,
                                                )
                                            }
                                            _ => usize::MAX,
                                        };
                                        (target_group, index)
                                    })
                            });
                        }
                        self.group_drop_rects = group_rects;

                        if let Some((group, index)) = drop_slot {
                            if let Some(indicator) =
                                shortcut_drop_indicator(group, index, &shortcut_rects, vertical)
                            {
                                ui.painter().rect_filled(
                                    indicator,
                                    egui::CornerRadius::same(2),
                                    Color32::from_white_alpha(230),
                                );
                            }
                        }

                        if has_group_content && !apps.is_empty() {
                            dock_separator(ui, vertical, settings.icon_size + ICON_CELL_PADDING);
                        }

                        for (path, windows, icon) in &apps {
                            let key = path_key(path);
                            let tooltip = if windows.len() == 1 {
                                windows[0].title.clone()
                            } else {
                                let word = match language {
                                    AppLanguage::French => "fenêtres de",
                                    AppLanguage::English => "windows for",
                                    AppLanguage::Spanish => "ventanas de",
                                };
                                format!("{} {word} {}", windows.len(), display_name(path))
                            };
                            let response = ui
                                .push_id(("active", &key), |ui| {
                                    icon_button(
                                        ui,
                                        icon.as_ref(),
                                        settings.icon_size,
                                        egui::Sense::click_and_drag(),
                                        icon_opacity,
                                    )
                                })
                                .inner
                                .on_hover_text(tooltip);
                            if response.drag_started() {
                                self.dragging_item =
                                    Some(DragItem::RunningApp { path: path.clone() });
                            }
                            if self
                                .window_preview
                                .as_ref()
                                .is_some_and(|preview| preview.app_key == key)
                            {
                                preview_anchor_rect = Some(response.rect);
                            }
                            if response.clicked() {
                                if windows.len() > 1 {
                                    if self
                                        .window_preview
                                        .as_ref()
                                        .is_some_and(|preview| preview.app_key == key)
                                    {
                                        preview_close_request = true;
                                    } else {
                                        preview_request = Some((
                                            key.clone(),
                                            display_name(path),
                                            windows.clone(),
                                        ));
                                        preview_request_anchor = Some(response.rect);
                                    }
                                } else {
                                    activate_window = windows.first().map(|window| window.window);
                                    preview_close_request = true;
                                }
                            }

                            let already_pinned: Vec<bool> = self
                                .config
                                .groups
                                .iter()
                                .map(|group| {
                                    group
                                        .shortcuts
                                        .iter()
                                        .any(|shortcut| path_key(&shortcut.path) == key)
                                })
                                .collect();
                            let _ = response.context_menu(|ui| {
                                show_scrollable_menu(ui, "running-app-context-menu", |ui| {
                                    if !windows.is_empty() {
                                        menu_section(ui, tr(language, "menu_section_application"));
                                        if windows.len() > 1 {
                                            menu_submenu(
                                                ui,
                                                MENU_ICON_WINDOWS,
                                                &format!(
                                                    "{} ({})",
                                                    tr(language, "open_windows"),
                                                    windows.len()
                                                ),
                                                |ui| {
                                                    show_scrollable_menu(
                                                        ui,
                                                        "open-windows-menu",
                                                        |ui| {
                                                            for (index, window) in
                                                                windows.iter().enumerate()
                                                            {
                                                                if menu_item(
                                                                    ui,
                                                                    MENU_ICON_WINDOWS,
                                                                    &format!(
                                                                        "{}: {}",
                                                                        index + 1,
                                                                        window.title
                                                                    ),
                                                                )
                                                                .clicked()
                                                                {
                                                                    activate_window =
                                                                        Some(window.window);
                                                                    ui.close_menu();
                                                                }
                                                            }
                                                        },
                                                    );
                                                },
                                            );
                                            menu_submenu(
                                                ui,
                                                MENU_ICON_CLOSE,
                                                tr(language, "close_window"),
                                                |ui| {
                                                    show_scrollable_menu(
                                                        ui,
                                                        "close-windows-menu",
                                                        |ui| {
                                                            for (index, window) in
                                                                windows.iter().enumerate()
                                                            {
                                                                if menu_item(
                                                                    ui,
                                                                    MENU_ICON_CLOSE,
                                                                    &format!(
                                                                        "{}: {}",
                                                                        index + 1,
                                                                        window.title
                                                                    ),
                                                                )
                                                                .clicked()
                                                                {
                                                                    close_window_request =
                                                                        Some(window.window);
                                                                    ui.close_menu();
                                                                }
                                                            }
                                                        },
                                                    );
                                                },
                                            );
                                        } else if let Some(window) = windows.first() {
                                            if menu_item(
                                                ui,
                                                MENU_ICON_CLOSE,
                                                tr(language, "close_this_window"),
                                            )
                                            .clicked()
                                            {
                                                close_window_request = Some(window.window);
                                                ui.close_menu();
                                            }
                                        }
                                        if menu_item(
                                            ui,
                                            MENU_ICON_REMOVE,
                                            tr(language, "close_all_windows"),
                                        )
                                        .clicked()
                                        {
                                            close_application_request = Some(path.clone());
                                            ui.close_menu();
                                        }
                                        if menu_item(
                                            ui,
                                            MENU_ICON_HIDE,
                                            tr(language, "hide_application"),
                                        )
                                        .clicked()
                                        {
                                            hide_app = Some(path.clone());
                                            ui.close_menu();
                                        }
                                    }
                                    menu_separator(ui);
                                    menu_section(ui, tr(language, "menu_section_organize"));
                                    menu_submenu(
                                        ui,
                                        MENU_ICON_PIN,
                                        tr(language, "add_to_group"),
                                        |ui| {
                                            show_scrollable_menu(ui, "add-to-group-menu", |ui| {
                                                for (index, name) in group_names.iter().enumerate()
                                                {
                                                    if menu_item_enabled(
                                                        ui,
                                                        MENU_ICON_GROUP,
                                                        name,
                                                        !already_pinned[index],
                                                    )
                                                    .clicked()
                                                    {
                                                        pin_request = Some((index, path.clone()));
                                                        ui.close_menu();
                                                    }
                                                }
                                            });
                                        },
                                    );
                                    if !hidden_apps.is_empty() {
                                        menu_separator(ui);
                                    }
                                    menu_section(ui, tr(language, "menu_section_dock"));
                                    hidden_apps_menu(
                                        ui,
                                        language,
                                        &hidden_apps,
                                        &mut show_app,
                                        &mut show_all_apps,
                                    );
                                    settings_menu(ui, settings, &mut settings_changes);
                                });
                            });
                            context_menu_open |= response.context_menu_opened();
                        }

                        if has_center_content && has_right_controls {
                            dock_separator(ui, vertical, settings.icon_size + ICON_CELL_PADDING);
                        } else if has_left_controls && has_right_controls {
                            dock_separator(ui, vertical, settings.icon_size + ICON_CELL_PADDING);
                        }
                        if right_power {
                            let (clicked, menu_open) =
                                show_power_control(ui, settings, &mut settings_changes);
                            context_menu_open |= menu_open;
                            if clicked {
                                self.shutdown_confirmation_open = true;
                            }
                        }
                        if right_information {
                            if right_power {
                                dock_separator(
                                    ui,
                                    vertical,
                                    settings.icon_size + ICON_CELL_PADDING,
                                );
                            }
                            context_menu_open |= show_information_controls(
                                ui,
                                settings,
                                settings.weather_side == DockSide::Right,
                                settings.show_clock,
                                current_weather.as_ref(),
                                weather_error.as_deref(),
                                current_time.as_deref(),
                                &mut settings_changes,
                            );
                        }
                        if right_system_control_count > 0 {
                            if right_power || right_information {
                                dock_separator(
                                    ui,
                                    vertical,
                                    settings.icon_size + ICON_CELL_PADDING,
                                );
                            }
                            let mut add_system_controls = |ui: &mut egui::Ui| {
                                if settings.show_wifi {
                                    let response =
                                        wifi_control(ui, settings.icon_size, icon_opacity);
                                    if response.clicked() {
                                        open_wifi_networks = true;
                                    }
                                    context_menu_open |= add_settings_context_menu(
                                        &response,
                                        settings,
                                        &mut settings_changes,
                                    );
                                }
                                if settings.show_volume {
                                    let response = volume_control(
                                        ui,
                                        settings,
                                        &mut volume,
                                        volume_error.as_deref(),
                                        &mut volume_change,
                                        icon_opacity,
                                    );
                                    context_menu_open |= add_settings_context_menu(
                                        &response,
                                        settings,
                                        &mut settings_changes,
                                    );
                                }
                            };
                            if vertical {
                                let spacing = ui.spacing().item_spacing.x;
                                let group_width = right_system_control_count as f32
                                    * SYSTEM_CONTROL_SIZE
                                    + right_system_control_count.saturating_sub(1) as f32 * spacing;
                                let left_offset =
                                    ((ui.available_width() - group_width) / 2.0).max(0.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(left_offset);
                                    add_system_controls(ui);
                                });
                            } else {
                                add_system_controls(ui);
                            }
                        }
                    },
                );
            });

        let _ = background_response.context_menu(|ui| {
            show_scrollable_menu(ui, "background-context-menu", |ui| {
                menu_section(ui, tr(language, "menu_section_dock"));
                hidden_apps_menu(
                    ui,
                    language,
                    &hidden_apps,
                    &mut show_app,
                    &mut show_all_apps,
                );
                settings_menu(ui, settings, &mut settings_changes);
            });
        });
        context_menu_open |= background_response.context_menu_opened();
        let secondary_released = ctx.input(|input| {
            input
                .pointer
                .button_released(egui::PointerButton::Secondary)
        });
        if secondary_released {
            if ctx.is_context_menu_open() {
                self.fallback_context_menu_position = None;
            } else {
                let origin = viewport_origin(ctx);
                self.fallback_context_menu_position = ctx
                    .input(|input| input.pointer.interact_pos())
                    .map(|position| position + origin);
            }
        }
        if let Some(global_position) = self.fallback_context_menu_position {
            let position = global_position - viewport_origin(ctx);
            let popup = egui::Area::new(egui::Id::new("fallback-application-context-menu"))
                .order(egui::Order::Foreground)
                .fixed_pos(position)
                .constrain(true)
                .default_width(ctx.style().spacing.menu_width)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                            show_scrollable_menu(ui, "fallback-context-menu", |ui| {
                                menu_section(ui, tr(language, "menu_section_dock"));
                                hidden_apps_menu(
                                    ui,
                                    language,
                                    &hidden_apps,
                                    &mut show_app,
                                    &mut show_all_apps,
                                );
                                settings_menu(ui, settings, &mut settings_changes);
                            });
                        });
                    });
                });
            let pointer_position = ctx.input(|input| input.pointer.interact_pos());
            let clicked_outside = ctx.input(|input| input.pointer.any_click())
                && pointer_position.is_some_and(|pointer_position| {
                    !popup.response.rect.contains(pointer_position)
                });
            let dismissed =
                ctx.input(|input| input.key_pressed(egui::Key::Escape)) || clicked_outside;
            let menu_action = settings_changes.has_changes()
                || settings_changes.settings_popup_anchor.is_some()
                || dismissed;
            if menu_action {
                self.fallback_context_menu_position = None;
            } else {
                context_menu_open = true;
            }
        }
        if open_wifi_networks {
            if let Err(error) = system_controls::open_wifi_networks() {
                show_error(&format!("{}: {error}", tr(language, "wifi_error")));
            }
        }
        if let Some(volume) = volume_change {
            match system_controls::set_master_volume(volume) {
                Ok(()) => {
                    self.system_volume = Some(volume);
                    self.system_volume_error = None;
                }
                Err(error) => {
                    self.system_volume_error = Some(error.clone());
                    show_error(&format!("{}: {error}", tr(language, "volume_error")));
                }
            }
        } else {
            self.system_volume = volume;
        }
        if settings.show_search {
            let query = self.search_query.trim();
            if !query.is_empty() {
                let matches: Vec<SearchApp> = self
                    .search_apps
                    .iter()
                    .filter(|app| search_app_matches(app, query))
                    .take(SEARCH_RESULTS_MAX_COUNT)
                    .cloned()
                    .collect();
                if search_enter_pressed {
                    search_launch_path = matches.first().map(|app| app.path.clone());
                }
                if let Some(rect) = search_rect {
                    let result_count = if self.search_error.is_some() || matches.is_empty() {
                        1
                    } else {
                        matches.len()
                    };
                    let screen_rect = ctx.screen_rect();
                    let popup_size = egui::vec2(
                        rect.width()
                            .min((screen_rect.width() - 2.0 * POPUP_SCREEN_MARGIN).max(1.0)),
                        search_results_height(result_count)
                            .min((screen_rect.height() - 2.0 * POPUP_SCREEN_MARGIN).max(1.0)),
                    );
                    let position = popup_position_for_dock(
                        settings.dock_position,
                        rect,
                        popup_size,
                        screen_rect,
                    );
                    egui::Area::new(egui::Id::new("application-search-results"))
                        .order(egui::Order::Foreground)
                        .fixed_pos(position)
                        .constrain(true)
                        .show(ctx, |ui| {
                            ui.set_width(popup_size.x);
                            egui::Frame::new()
                                .fill(dock_background_color_with_opacity(
                                    settings.background_color,
                                    settings.background_opacity,
                                ))
                                .corner_radius(egui::CornerRadius::same(12))
                                .stroke(egui::Stroke::new(1.0, Color32::from_white_alpha(48)))
                                .inner_margin(egui::Margin::same(6))
                                .show(ui, |ui| {
                                    ui.set_width((popup_size.x - 12.0).max(1.0));
                                    ui.spacing_mut().item_spacing.y = SEARCH_RESULTS_ROW_SPACING;
                                    if let Some(error) = &self.search_error {
                                        ui.add_sized(
                                            [ui.available_width(), SEARCH_RESULT_ROW_HEIGHT],
                                            egui::Label::new(format!(
                                                "{}: {error}",
                                                tr(language, "search_error")
                                            )),
                                        );
                                    } else if matches.is_empty() {
                                        ui.add_sized(
                                            [ui.available_width(), SEARCH_RESULT_ROW_HEIGHT],
                                            egui::Label::new(tr(language, "no_apps_found")),
                                        );
                                    } else {
                                        egui::ScrollArea::vertical()
                                            .max_height(
                                                SEARCH_RESULTS_MAX_HEIGHT
                                                    - SEARCH_RESULTS_FRAME_PADDING,
                                            )
                                            .show(ui, |ui| {
                                                for app in &matches {
                                                    if ui
                                                        .add_sized(
                                                            [
                                                                ui.available_width(),
                                                                SEARCH_RESULT_ROW_HEIGHT,
                                                            ],
                                                            egui::Button::new(&app.name),
                                                        )
                                                        .clicked()
                                                    {
                                                        search_launch_path = Some(app.path.clone());
                                                    }
                                                }
                                            });
                                    }
                                });
                        });
                }
            }
        }
        let mut close_preview = preview_close_request;
        if let Some(preview) = self.window_preview.as_mut() {
            if let Some(anchor) = preview_anchor_rect {
                let action =
                    show_window_preview(ctx, preview, anchor, settings.dock_position, language);
                if let Some(window) = action.activate_window {
                    activate_window = Some(window);
                }
                close_preview |= action.dismiss;
            } else {
                close_preview = true;
            }
        }
        if close_preview {
            self.window_preview = None;
        }
        if let Some((app_key, app_name, windows)) = preview_request {
            self.window_preview = Some(create_window_preview(app_key, app_name, windows));
            ctx.request_repaint();
        }
        let opened_settings_popup = settings_changes.settings_popup_anchor.is_some();
        if opened_settings_popup {
            self.settings_popup_has_focus
                .store(false, Ordering::Relaxed);
            self.settings_popup_dismiss.store(false, Ordering::Relaxed);
            self.settings_popup_position = None;
        }
        let settings_viewport_id = egui::ViewportId::from_hash_of("settings-popup");
        let settings_popup_anchor = if settings_changes.dismiss_settings_popup {
            None
        } else {
            settings_changes
                .settings_popup_anchor
                .or(self.settings_popup_anchor)
        };
        // Un clic dans la fenetre du dock est forcement hors du panneau
        // (fenetre distincte) : on demande au panneau de se fermer.
        if !opened_settings_popup
            && settings_popup_anchor.is_some()
            && ctx.input(|input| input.pointer.any_click())
        {
            self.settings_popup_dismiss.store(true, Ordering::Relaxed);
            ctx.request_repaint_of(settings_viewport_id);
        }
        if let Some(anchor) = settings_popup_anchor {
            let safe_rect = popup_safe_rect(ctx);
            let popup_size = egui::vec2(
                (SETTINGS_PANEL_WIDTH + 2.0 * POPUP_SCREEN_MARGIN).min(available_dock_width(ctx)),
                SETTINGS_PANEL_HEIGHT.min(popup_height_for_available_screen(
                    available_dock_height(ctx),
                )),
            );
            let parent_origin = ctx.input(|input| {
                input
                    .viewport()
                    .outer_rect
                    .map_or(egui::Pos2::ZERO, |rect| rect.min)
            });
            let position = if let Some(position) = self.settings_popup_position {
                position
            } else {
                let position = popup_position_for_anchor(
                    anchor.translate(parent_origin.to_vec2()),
                    popup_size,
                    safe_rect,
                );
                self.settings_popup_position = Some(position);
                position
            };
            let viewport_id = settings_viewport_id;
            let popup_settings = SettingsState {
                icon_size: settings.icon_size,
                vertical_padding: settings.vertical_padding,
                background_opacity: settings.background_opacity,
                icon_opacity: settings.icon_opacity,
                show_clock: settings.show_clock,
                show_weather: settings.show_weather,
                show_wifi: settings.show_wifi,
                show_volume: settings.show_volume,
                show_shutdown_button: settings.show_shutdown_button,
                show_search: settings.show_search,
                shutdown_side: settings.shutdown_side,
                weather_side: settings.weather_side,
                dock_position: settings.dock_position,
                background_color: settings.background_color,
                startup_enabled: settings.startup_enabled,
                always_on_top: settings.always_on_top,
                weather_city: "",
                language: settings.language,
            };
            let popup_weather_city = settings.weather_city.to_owned();
            let popup_group_names: Vec<String> = group_names.clone();
            let initial_city_draft = weather_city_draft.clone();
            let changes_sender = self.settings_changes_sender.clone();
            let has_focus = self.settings_popup_has_focus.clone();
            let dismiss_requested = self.settings_popup_dismiss.clone();
            let window_level = if settings.always_on_top {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            };
            let viewport_builder = egui::ViewportBuilder::default()
                .with_title("Dock Windows Settings")
                .with_inner_size(popup_size)
                .with_position(position)
                .with_decorations(false)
                .with_resizable(false)
                .with_window_level(window_level)
                .with_transparent(true)
                .with_active(true);
            ctx.show_viewport_deferred(
                viewport_id,
                viewport_builder,
                move |popup_ctx, viewport_class| {
                    apply_design(popup_ctx);
                    let mut popup_changes = SettingsChanges::default();
                    let mut city_draft = initial_city_draft.clone();
                    let popup_settings = SettingsState {
                        weather_city: &popup_weather_city,
                        ..popup_settings
                    };
                    let mut show_controls = |ui: &mut egui::Ui| {
                        settings_controls(
                            ui,
                            popup_settings,
                            &mut city_draft,
                            &popup_group_names,
                            &mut popup_changes,
                        );
                    };
                    if viewport_class == egui::ViewportClass::Embedded {
                        egui::Window::new("Dock Windows Settings")
                            .title_bar(false)
                            .resizable(false)
                            .collapsible(false)
                            .show(popup_ctx, |ui| show_controls(ui));
                    } else {
                        egui::CentralPanel::default()
                            .frame(egui::Frame::menu(&popup_ctx.style()).inner_margin(14))
                            .show(popup_ctx, |ui| show_controls(ui));
                    }
                    if city_draft != initial_city_draft {
                        popup_changes.weather_city_draft = Some(city_draft);
                    }
                    let dismissed = popup_ctx.input(|input| {
                        input.viewport().close_requested() || input.key_pressed(egui::Key::Escape)
                    });
                    let focused = popup_ctx.input(|input| input.focused);
                    let lost_focus = has_focus.load(Ordering::Relaxed) && !focused;
                    if focused {
                        has_focus.store(true, Ordering::Relaxed);
                    }
                    let close_requested = dismiss_requested.swap(false, Ordering::Relaxed);
                    if dismissed || lost_focus || close_requested {
                        // Masque la fenetre immediatement : la fenetre principale peut
                        // ne pas etre en cours de rafraichissement et ne traiterait
                        // donc pas tout de suite la demande de fermeture.
                        popup_ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                        popup_ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        popup_ctx.request_repaint_of(egui::ViewportId::ROOT);
                        popup_changes.dismiss_settings_popup = true;
                    }
                    popup_ctx.request_repaint_after(Duration::from_millis(100));
                    if popup_changes.has_changes()
                        || popup_changes.weather_city_draft.is_some()
                        || popup_changes.dismiss_settings_popup
                    {
                        if let Err(error) = changes_sender.send(popup_changes) {
                            show_error(&format!("Failed to update settings: {error}"));
                        }
                        // Recalcule le dock tout de suite : les curseurs de
                        // taille, d'espacement et d'opacite restent ainsi en
                        // apercu temps reel.
                        popup_ctx.request_repaint_of(egui::ViewportId::ROOT);
                    }
                },
            );
        }
        if settings_popup_anchor != self.settings_popup_anchor {
            if settings_popup_anchor.is_none() {
                ctx.send_viewport_cmd_to(settings_viewport_id, egui::ViewportCommand::Close);
                self.settings_popup_position = None;
            }
            self.settings_popup_anchor = settings_popup_anchor;
            ctx.request_repaint();
        }
        if self.context_menu_open != context_menu_open {
            ctx.request_repaint();
        }
        self.context_menu_open = context_menu_open;

        self.weather_city_draft = weather_city_draft;
        if let Some(city_draft) = settings_changes.weather_city_draft.take() {
            self.weather_city_draft = city_draft;
        }
        if let Some(path) = launch_path {
            self.launch(&path);
        }
        if let Some(path) = search_launch_path {
            self.search_query.clear();
            self.launch(&path);
        }
        if let Some((group_index, index)) = remove_shortcut {
            self.config.groups[group_index].shortcuts.remove(index);
            changed = true;
        }
        if let Some((from_group, index, to_group)) = move_shortcut_request {
            if self.config.move_shortcut(from_group, index, to_group) {
                changed = true;
            }
        }
        if pointer_released && self.dragging_item.is_some() {
            if let Some((to_group, to_index)) = drop_slot {
                if let Some(item) = self.dragging_item.take() {
                    let modified = match item {
                        DragItem::Shortcut {
                            group: from_group,
                            index: from_index,
                        } => self
                            .config
                            .move_shortcut_to_position(from_group, from_index, to_group, to_index),
                        DragItem::RunningApp { path } => {
                            self.config.add_shortcut_to_group(to_group, path)
                        }
                    };
                    changed |= modified;
                }
            } else {
                self.dragging_item = None;
            }
        }
        if let Some(window) = activate_window {
            active_apps::activate(window);
        }
        if let Some(window) = close_window_request {
            if let Err(error) = active_apps::close_window(window) {
                show_error(&localize_active_apps_error(language, &error));
            }
        }
        if let Some(path) = close_application_request {
            if let Err(error) = active_apps::close_application(&path) {
                show_error(&localize_active_apps_error(language, &error));
            }
        }
        if let Some(icon_size) = settings_changes.icon_size {
            let icon_size = icon_size.clamp(MIN_ICON_SIZE, MAX_ICON_SIZE);
            if self.config.icon_size != icon_size {
                self.config.icon_size = icon_size;
                changed = true;
            }
        }
        if let Some(vertical_padding) = settings_changes.vertical_padding {
            let vertical_padding =
                vertical_padding.clamp(MIN_VERTICAL_PADDING, MAX_VERTICAL_PADDING);
            if self.config.vertical_padding != vertical_padding {
                self.config.vertical_padding = vertical_padding;
                changed = true;
            }
        }
        if let Some(background_opacity) = settings_changes.background_opacity {
            let background_opacity = background_opacity.clamp(MIN_OPACITY, MAX_OPACITY);
            if self.config.background_opacity != background_opacity {
                self.config.background_opacity = background_opacity;
                changed = true;
            }
        }
        if let Some(icon_opacity) = settings_changes.icon_opacity {
            let icon_opacity = icon_opacity.clamp(MIN_OPACITY, MAX_OPACITY);
            if self.config.icon_opacity != icon_opacity {
                self.config.icon_opacity = icon_opacity;
                changed = true;
            }
        }
        if let Some(show_clock) = settings_changes.show_clock {
            if self.config.show_clock != show_clock {
                self.config.show_clock = show_clock;
                changed = true;
            }
        }
        if let Some(show_weather) = settings_changes.show_weather {
            if self.config.show_weather != show_weather {
                self.config.show_weather = show_weather;
                changed = true;
            }
        }
        if let Some(show_wifi) = settings_changes.show_wifi {
            if self.config.show_wifi != show_wifi {
                self.config.show_wifi = show_wifi;
                changed = true;
            }
        }
        if let Some(show_volume) = settings_changes.show_volume {
            if self.config.show_volume != show_volume {
                self.config.show_volume = show_volume;
                changed = true;
            }
        }
        if let Some(show_shutdown_button) = settings_changes.show_shutdown_button {
            if self.config.show_shutdown_button != show_shutdown_button {
                self.config.show_shutdown_button = show_shutdown_button;
                changed = true;
            }
        }
        if let Some(show_search) = settings_changes.show_search {
            if self.config.show_search != show_search {
                self.config.show_search = show_search;
                changed = true;
            }
        }
        if let Some(shutdown_side) = settings_changes.shutdown_side {
            if self.config.shutdown_side != shutdown_side {
                self.config.shutdown_side = shutdown_side;
                changed = true;
            }
        }
        if let Some(weather_side) = settings_changes.weather_side {
            if self.config.weather_side != weather_side {
                self.config.weather_side = weather_side;
                changed = true;
            }
        }
        if let Some(dock_position) = settings_changes.dock_position {
            if self.config.dock_position != dock_position {
                self.config.dock_position = dock_position;
                ctx.memory_mut(|memory| memory.close_popup());
                self.context_menu_open = false;
                changed = true;
            }
        }
        if let Some(background_color) = settings_changes.background_color {
            if self.config.background_color != background_color {
                self.config.background_color = background_color;
                changed = true;
            }
        }
        if let Some(city) = settings_changes.weather_city {
            if self.config.weather_city != city {
                self.config.weather_city = city.clone();
                self.weather_city_draft = city.clone();
                self.weather = None;
                self.weather_error = None;
                self.weather_request_in_flight = false;
                self.weather_request_city = None;
                self.last_weather_request = None;
                changed = true;
            }
        }
        if let Some(language) = settings_changes.language {
            if self.config.language != language {
                self.config.language = language;
                changed = true;
            }
        }
        if let Some(always_on_top) = settings_changes.always_on_top {
            if self.config.always_on_top != always_on_top {
                self.config.always_on_top = always_on_top;
                let window_level = if always_on_top {
                    egui::WindowLevel::AlwaysOnTop
                } else {
                    egui::WindowLevel::Normal
                };
                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(window_level));
                changed = true;
            }
        }
        if let Some(startup_enabled) = settings_changes.startup_enabled {
            if startup_enabled != self.startup_enabled {
                match startup::set_enabled(startup_enabled) {
                    Ok(()) => self.startup_enabled = startup_enabled,
                    Err(error) => {
                        show_error(&format!("{}: {error}", tr(language, "startup_error")))
                    }
                }
            }
        }
        if let Some((group_index, path)) = pin_request {
            if self.config.add_shortcut_to_group(group_index, path) {
                changed = true;
            }
        }
        if let Some(name) = settings_changes.create_group.take() {
            if self.config.add_group(&name).is_ok() {
                changed = true;
            }
        }
        if let Some((index, name)) = settings_changes.rename_group.take() {
            if self.config.rename_group(index, &name).is_ok() {
                changed = true;
            }
        }
        if let Some(index) = settings_changes.delete_group.take() {
            if self.config.remove_group(index) {
                changed = true;
            }
        }
        if settings_changes.export_config {
            settings_changes.export_config = false;
            self.export_config();
        }
        if settings_changes.import_config {
            settings_changes.import_config = false;
            self.import_config(ctx);
        }
        if let Some(path) = hide_app {
            if self.config.hide_app(&path) {
                let key = path_key(&path);
                self.running_apps.retain(|app| path_key(&app.path) != key);
                changed = true;
            }
        }
        if let Some(key) = show_app {
            if self.config.show_app(&key) {
                self.refresh_active_apps(ctx);
                changed = true;
            }
        }
        if show_all_apps && self.config.show_all_apps() {
            self.refresh_active_apps(ctx);
            changed = true;
        }
        if settings_changes.quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        changed |= std::mem::take(&mut self.settings_dirty);
        if changed {
            self.settings_persist_pending = true;
        }
        if self.settings_persist_pending
            && self
                .last_settings_persist
                .map_or(true, |last| last.elapsed() >= Duration::from_millis(250))
        {
            self.settings_persist_pending = false;
            self.last_settings_persist = Some(Instant::now());
            let _ = self.persist();
        }
    }
}

impl eframe::App for DockApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Tant que l'assistant de premier demarrage n'est pas termine, aucune
        // configuration ne doit etre ecrite, sinon il ne reapparaitrait plus.
        if self.setup.is_none() {
            let _ = write_config(&self.config_path, &self.config);
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.setup.is_some() {
            self.show_setup(ctx);
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        while let Ok(changes) = self.settings_changes_receiver.try_recv() {
            self.pending_settings.merge(changes);
        }
        self.apply_live_settings();
        self.refresh_weather();
        if self.config.show_volume
            && self
                .last_volume_refresh
                .map_or(true, |last| last.elapsed() >= Duration::from_secs(1))
        {
            match system_controls::master_volume() {
                Ok(volume) => {
                    self.system_volume = Some(volume);
                    self.system_volume_error = None;
                }
                Err(error) => self.system_volume_error = Some(error),
            }
            self.last_volume_refresh = Some(Instant::now());
        }
        if self
            .last_active_refresh
            .map_or(true, |last| last.elapsed() >= Duration::from_secs(2))
        {
            self.refresh_active_apps(ctx);
            self.last_active_refresh = Some(Instant::now());
        }
        self.resize_to_content(ctx);

        ctx.request_repaint_after(Duration::from_millis(500));

        let dropped_files = ctx.input(|input| input.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            // Les fichiers deposes atterrissent dans le groupe survole, sinon dans
            // le groupe par defaut.
            let target = ctx
                .input(|input| input.pointer.latest_pos())
                .and_then(|pointer| {
                    self.group_drop_rects
                        .iter()
                        .position(|rect| rect.is_some_and(|rect| rect.contains(pointer)))
                })
                .unwrap_or(self.config.selected_group);
            self.import_dropped_files(dropped_files, target);
        }
        let vertical = self.config.dock_position.is_vertical();
        let DockLayout {
            area_width,
            area_height,
            wrap: wrap_horizontal_layout,
        } = self.dock_layout;
        let vertical_padding = self.config.vertical_padding as i8;
        let (dock_anchor, dock_offset) = dock_area_anchor(self.config.dock_position);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |_ui| {
                let background_response = _ui.interact(
                    _ui.max_rect(),
                    egui::Id::new("application-background-context-menu"),
                    egui::Sense::click(),
                );
                egui::Area::new(egui::Id::new("dock"))
                    .default_size([area_width, area_height])
                    .anchor(dock_anchor, dock_offset)
                    .show(ctx, |ui| {
                        ui.set_width(area_width);
                        ui.set_min_height(area_height);
                        let dock_radius = ((area_height * 0.5).clamp(8.0, 28.0)) as u8;
                        egui::Frame::new()
                            .fill(dock_background_color_with_opacity(
                                self.config.background_color,
                                self.config.background_opacity,
                            ))
                            .corner_radius(egui::CornerRadius::same(dock_radius))
                            .stroke(egui::Stroke::new(
                                1.0,
                                Color32::from_white_alpha(scaled_alpha(
                                    35,
                                    self.config.background_opacity,
                                )),
                            ))
                            .inner_margin(egui::Margin {
                                left: 8,
                                right: 8,
                                top: vertical_padding,
                                bottom: vertical_padding,
                            })
                            .show(ui, |ui| {
                                self.show_dock(
                                    ui,
                                    ctx,
                                    &background_response,
                                    vertical,
                                    wrap_horizontal_layout,
                                );
                            });
                    });
            });
        show_shutdown_confirmation(
            ctx,
            &mut self.shutdown_confirmation_open,
            self.config.language,
        );
    }
}

fn show_shutdown_confirmation(ctx: &egui::Context, is_open: &mut bool, language: AppLanguage) {
    if !*is_open {
        return;
    }

    let mut window_open = *is_open;
    let mut confirmed = false;
    let mut cancelled = false;
    egui::Window::new(tr(language, "shutdown_title"))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .open(&mut window_open)
        .show(ctx, |ui| {
            ui.label(tr(language, "shutdown_question"));
            ui.horizontal(|ui| {
                if ui.button(tr(language, "cancel")).clicked() {
                    cancelled = true;
                }
                if ui.button(tr(language, "shutdown_now")).clicked() {
                    confirmed = true;
                }
            });
        });

    *is_open = window_open && !confirmed && !cancelled;
    if confirmed {
        if let Err(error) = request_shutdown() {
            show_error(&format!("{}: {error}", tr(language, "shutdown_error")));
        }
    }
}

fn request_shutdown() -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

        let status = Command::new("shutdown.exe")
            .args(["/s", "/t", "0"])
            .creation_flags(CREATE_NO_WINDOW)
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "shutdown.exe a retourné le code {}",
                status.code().unwrap_or(-1)
            )))
        }
    }

    #[cfg(not(windows))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "L'arrêt de l'ordinateur est disponible uniquement sous Windows.",
        ))
    }
}

fn config_path() -> io::Result<PathBuf> {
    let app_data = std::env::var_os("APPDATA").ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "La variable APPDATA est absente.")
    })?;
    Ok(PathBuf::from(app_data)
        .join("DockWindows")
        .join("config.json"))
}

fn config_backup_path(path: &Path) -> PathBuf {
    path.with_file_name("config.backup.json")
}

/// Lit une configuration ; `None` si le fichier est absent ou illisible en tant
/// que JSON, afin de pouvoir se rabattre sur la copie de secours.
fn read_config(path: &Path) -> io::Result<Option<DockConfig>> {
    match fs::read(path) {
        Ok(contents) => Ok(parse_config(&contents)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Decode une configuration en ignorant un eventuel BOM UTF-8 ajoute par un
/// editeur de texte.
fn parse_config(contents: &[u8]) -> Option<DockConfig> {
    let contents = contents
        .strip_prefix(&[0xEF, 0xBB, 0xBF])
        .unwrap_or(contents);
    serde_json::from_slice(contents).ok()
}

/// Charge la configuration et se rabat sur la copie de secours si le fichier
/// principal a disparu ou ete abime (par exemple apres une reinstallation).
fn load_config_with_backup(path: &Path) -> io::Result<DockConfig> {
    if let Some(config) = read_config(path)? {
        return Ok(config);
    }
    match read_config(&config_backup_path(path))? {
        Some(config) => {
            let _ = write_config(path, &config);
            Ok(config)
        }
        None => Ok(DockConfig::default()),
    }
}

/// Enregistre la configuration : ecriture atomique puis copie de secours, pour
/// qu'une interruption ne perde jamais les raccourcis et les groupes.
fn write_config(path: &Path, config: &DockConfig) -> Result<(), Box<dyn Error>> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Chemin de configuration invalide."))?;
    fs::create_dir_all(parent)?;
    let contents = serde_json::to_vec_pretty(config)?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, contents)?;
    if path.exists() {
        let _ = fs::copy(path, config_backup_path(path));
    }
    fs::rename(&temporary, path)?;
    Ok(())
}

fn display_name(path: &Path) -> String {
    let name = path
        .file_stem()
        .or_else(|| path.file_name())
        .unwrap_or_default()
        .to_string_lossy();
    if name.is_empty() {
        path.display().to_string()
    } else {
        name.into_owned()
    }
}

fn localize_active_apps_error(language: AppLanguage, error: &str) -> String {
    let key = match error {
        "Windows n'a pas accepté la demande de fermeture." => "close_denied",
        "La fermeture native n'est prise en charge que sous Windows." => "close_unsupported",
        _ => return error.to_owned(),
    };
    tr(language, key).to_owned()
}

fn start_menu_apps() -> io::Result<Vec<SearchApp>> {
    let mut apps = Vec::new();
    for variable in ["APPDATA", "PROGRAMDATA"] {
        if let Some(root) = std::env::var_os(variable) {
            let programs = PathBuf::from(root)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs");
            collect_start_menu_apps(&programs, &mut apps)?;
        }
    }

    apps.sort_by_key(|app| app.name.to_lowercase());
    let mut seen = HashSet::new();
    apps.retain(|app| seen.insert(path_key(&app.path)));
    Ok(apps)
}

fn collect_start_menu_apps(directory: &Path, apps: &mut Vec<SearchApp>) -> io::Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_start_menu_apps(&path, apps)?;
        } else if file_type.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"))
        {
            apps.push(SearchApp {
                name: display_name(&path),
                path,
            });
        }
    }
    Ok(())
}

fn search_app_matches(app: &SearchApp, query: &str) -> bool {
    app.name
        .to_lowercase()
        .contains(&query.trim().to_lowercase())
}

/// Langue de l'interface Windows, pour pre-selectionner l'assistant.
fn system_language() -> AppLanguage {
    #[cfg(windows)]
    {
        let primary =
            unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() } & 0x3FF;
        return match primary {
            0x0C => AppLanguage::French,
            0x0A => AppLanguage::Spanish,
            _ => AppLanguage::English,
        };
    }

    #[cfg(not(windows))]
    {
        AppLanguage::English
    }
}

fn taskbar_shortcuts() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(app_data) = std::env::var_os("APPDATA") {
        let directory = PathBuf::from(app_data)
            .join("Microsoft")
            .join("Internet Explorer")
            .join("Quick Launch")
            .join("User Pinned")
            .join("TaskBar");
        let _ = collect_lnk_files(&directory, false, &mut paths);
    }
    finish_shortcut_list(paths)
}

#[cfg(windows)]
fn desktop_shortcuts() -> Vec<PathBuf> {
    use windows_sys::Win32::UI::Shell::{FOLDERID_Desktop, FOLDERID_PublicDesktop};

    let mut paths = Vec::new();
    for folder in [
        known_folder(&FOLDERID_Desktop),
        known_folder(&FOLDERID_PublicDesktop),
    ]
    .into_iter()
    .flatten()
    {
        let _ = collect_lnk_files(&folder, false, &mut paths);
    }
    finish_shortcut_list(paths)
}

#[cfg(not(windows))]
fn desktop_shortcuts() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(windows)]
fn known_folder(id: &windows_sys::core::GUID) -> Option<PathBuf> {
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::SHGetKnownFolderPath;

    let mut raw: *mut u16 = std::ptr::null_mut();
    let result = unsafe { SHGetKnownFolderPath(id, 0, std::ptr::null_mut(), &mut raw) };
    if result < 0 || raw.is_null() {
        return None;
    }
    let length = unsafe { (0..).take_while(|&index| *raw.add(index) != 0).count() };
    let path = unsafe { String::from_utf16_lossy(std::slice::from_raw_parts(raw, length)) };
    unsafe { CoTaskMemFree(raw as *const _) };
    Some(PathBuf::from(path))
}

fn collect_lnk_files(
    directory: &Path,
    recursive: bool,
    paths: &mut Vec<PathBuf>,
) -> io::Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if recursive {
                collect_lnk_files(&path, true, paths)?;
            }
        } else if file_type.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"))
        {
            paths.push(path);
        }
    }
    Ok(())
}

fn finish_shortcut_list(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.retain(|path| !is_dock_shortcut(path));
    paths.sort_by_key(|path| display_name(path).to_lowercase());
    let mut seen = HashSet::new();
    paths.retain(|path| seen.insert(path_key(path)));
    paths
}

/// Evite d'epingler le raccourci de Dock Windows lui-meme, cree par le MSI.
fn is_dock_shortcut(path: &Path) -> bool {
    display_name(path).to_lowercase().contains("dock windows")
}

fn setup_groups(
    language: AppLanguage,
    taskbar: &[PathBuf],
    desktop: &[PathBuf],
    start_menu: &[PathBuf],
    import_taskbar: bool,
    import_desktop: bool,
    import_start_menu: bool,
) -> Vec<ShortcutGroup> {
    let mut groups = Vec::new();
    let mut seen = HashSet::new();
    for (enabled, name_key, paths) in [
        (import_taskbar, "setup_taskbar", taskbar),
        (import_desktop, "setup_desktop", desktop),
        (import_start_menu, "setup_start_menu", start_menu),
    ] {
        if !enabled {
            continue;
        }
        let shortcuts: Vec<Shortcut> = paths
            .iter()
            .filter(|path| seen.insert(path_key(path)))
            .cloned()
            .map(|path| Shortcut { path })
            .collect();
        if !shortcuts.is_empty() {
            groups.push(ShortcutGroup {
                name: tr(language, name_key).to_owned(),
                shortcuts,
            });
        }
    }
    if groups.is_empty() {
        groups.push(ShortcutGroup {
            name: tr(language, "setup_group_default").to_owned(),
            shortcuts: Vec::new(),
        });
    }
    groups
}

fn setup_summary(language: AppLanguage, count: usize) -> String {
    let key = if count == 0 {
        "setup_summary_none"
    } else if count == 1 {
        "setup_summary_one"
    } else {
        "setup_summary_many"
    };
    tr(language, key).replace("{count}", &count.to_string())
}

fn setup_source_row(ui: &mut egui::Ui, selected: &mut bool, icon: &str, label: &str, count: usize) {
    ui.horizontal(|ui| {
        ui.add_enabled_ui(count > 0, |ui| {
            ui.checkbox(selected, menu_label(icon, label));
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(count.to_string())
                    .size(11.0)
                    .color(menu_muted_color()),
            );
        });
    });
}

fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let accent = Color32::from_rgb(72, 92, 190);
    let accent_hover = Color32::from_rgb(88, 110, 220);
    ui.scope(|ui| {
        let visuals = ui.visuals_mut();
        visuals.widgets.inactive.weak_bg_fill = accent;
        visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, Color32::WHITE);
        visuals.widgets.hovered.weak_bg_fill = accent_hover;
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, Color32::WHITE);
        visuals.widgets.active.weak_bg_fill = accent_hover;
        visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, Color32::WHITE);
        ui.add(
            egui::Button::new(egui::RichText::new(text).strong())
                .corner_radius(egui::CornerRadius::same(9))
                .min_size(egui::vec2(140.0, 36.0)),
        )
    })
    .inner
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}

/// Une application deja epinglee dans le groupe courant ne doit pas apparaitre
/// une seconde fois parmi les applications actives.
fn count_visible_running_apps(apps: &[RunningApp], pinned: &[String]) -> usize {
    apps.iter()
        .filter(|app| !pinned.contains(&path_key(&app.path)))
        .count()
}

fn group_running_apps(apps: Vec<active_apps::ActiveApplication>) -> Vec<RunningApp> {
    let mut grouped = Vec::new();
    let mut app_indexes = HashMap::new();

    for app in apps {
        let key = app.path_key();
        let index = if let Some(index) = app_indexes.get(&key) {
            *index
        } else {
            let index = grouped.len();
            grouped.push(RunningApp {
                path: app.path.clone(),
                windows: Vec::new(),
                icon: None,
            });
            app_indexes.insert(key, index);
            index
        };
        grouped[index].windows.push(RunningWindow {
            title: app.title,
            window: app.window,
        });
    }

    grouped
}

#[derive(Default)]
struct WindowPreviewAction {
    activate_window: Option<isize>,
    dismiss: bool,
}

fn create_window_preview(
    app_key: String,
    app_name: String,
    windows: Vec<RunningWindow>,
) -> WindowPreviewGroup {
    let destination = active_apps::current_window_handle();
    let mut thumbnails = Vec::with_capacity(windows.len());
    let mut thumbnail_errors = Vec::with_capacity(windows.len());

    for window in &windows {
        let registration = destination
            .ok_or_else(|| "Could not find the dock window for the preview.".to_owned())
            .and_then(|destination| {
                window_thumbnails::Thumbnail::register(destination, window.window)
            });
        match registration {
            Ok(thumbnail) => {
                thumbnails.push(Some(thumbnail));
                thumbnail_errors.push(None);
            }
            Err(error) => {
                thumbnails.push(None);
                thumbnail_errors.push(Some(error));
            }
        }
    }

    WindowPreviewGroup {
        app_key,
        app_name,
        windows,
        thumbnails,
        thumbnail_errors,
    }
}

fn popup_position_for_dock(
    dock_position: DockPosition,
    anchor: egui::Rect,
    popup_size: egui::Vec2,
    screen_rect: egui::Rect,
) -> egui::Pos2 {
    let safe_rect = screen_rect;
    let popup_size = egui::vec2(
        popup_size.x.min(safe_rect.width().max(1.0)),
        popup_size.y.min(safe_rect.height().max(1.0)),
    );
    let centered_x = anchor.center().x - popup_size.x / 2.0;
    let centered_y = anchor.center().y - popup_size.y / 2.0;
    let (preferred, alternate) = match dock_position {
        DockPosition::Bottom => (
            egui::pos2(centered_x, anchor.top() - popup_size.y - POPUP_GAP),
            egui::pos2(centered_x, anchor.bottom() + POPUP_GAP),
        ),
        DockPosition::Top => (
            egui::pos2(centered_x, anchor.bottom() + POPUP_GAP),
            egui::pos2(centered_x, anchor.top() - popup_size.y - POPUP_GAP),
        ),
        DockPosition::Left => (
            egui::pos2(anchor.right() + POPUP_GAP, centered_y),
            egui::pos2(anchor.left() - popup_size.x - POPUP_GAP, centered_y),
        ),
        DockPosition::Right => (
            egui::pos2(anchor.left() - popup_size.x - POPUP_GAP, centered_y),
            egui::pos2(anchor.right() + POPUP_GAP, centered_y),
        ),
    };
    let fits = |position: egui::Pos2| {
        position.x >= safe_rect.left()
            && position.y >= safe_rect.top()
            && position.x + popup_size.x <= safe_rect.right()
            && position.y + popup_size.y <= safe_rect.bottom()
    };
    let position = if fits(preferred) {
        preferred
    } else if fits(alternate) {
        alternate
    } else {
        preferred
    };
    egui::pos2(
        position.x.clamp(
            safe_rect.left(),
            (safe_rect.right() - popup_size.x).max(safe_rect.left()),
        ),
        position.y.clamp(
            safe_rect.top(),
            (safe_rect.bottom() - popup_size.y).max(safe_rect.top()),
        ),
    )
}

fn available_popup_height(ctx: &egui::Context) -> f32 {
    popup_height_for_available_screen(available_dock_height(ctx))
}

fn popup_safe_rect(ctx: &egui::Context) -> egui::Rect {
    monitor_work_area(ctx)
}

fn popup_position_for_anchor(
    anchor: egui::Rect,
    popup_size: egui::Vec2,
    safe_rect: egui::Rect,
) -> egui::Pos2 {
    let safe_rect = safe_rect.shrink(POPUP_SCREEN_MARGIN);
    let right = anchor.right() + POPUP_GAP;
    let left = anchor.left() - popup_size.x - POPUP_GAP;
    let x = if right + popup_size.x <= safe_rect.right() {
        right
    } else {
        left
    };
    egui::pos2(
        x.clamp(
            safe_rect.left(),
            (safe_rect.right() - popup_size.x).max(safe_rect.left()),
        ),
        anchor.top().clamp(
            safe_rect.top(),
            (safe_rect.bottom() - popup_size.y).max(safe_rect.top()),
        ),
    )
}

fn popup_height_for_available_screen(available_height: f32) -> f32 {
    (available_height - 2.0 * POPUP_SCREEN_MARGIN).max(1.0)
}

fn menu_label(icon: &str, label: &str) -> String {
    format!("{icon}   {label}")
}

/// Entree de menu avec une icone, alignee sur toute la largeur du menu.
fn menu_item(ui: &mut egui::Ui, icon: &str, label: &str) -> egui::Response {
    ui.add(egui::Button::new(menu_label(icon, label)))
}

fn menu_item_enabled(ui: &mut egui::Ui, icon: &str, label: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(enabled, egui::Button::new(menu_label(icon, label)))
}

/// Sous-menu avec une icone ; il s'ouvre au survol comme les menus d'egui.
fn menu_submenu(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    ui.menu_button(menu_label(icon, label), add_contents)
        .response
}

/// Petit intitule de section, en majuscules discretes, pour aerer les menus.
fn menu_section(ui: &mut egui::Ui, label: &str) {
    ui.add_space(3.0);
    ui.label(
        egui::RichText::new(label.to_uppercase())
            .size(10.0)
            .strong()
            .color(menu_muted_color()),
    );
    ui.add_space(1.0);
}

/// Separateur pleine largeur, plus fin et plus discret que celui d'egui.
fn menu_separator(ui: &mut egui::Ui) {
    ui.add_space(4.0);
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().hline(
        rect.x_range().shrink(6.0),
        rect.center().y,
        egui::Stroke::new(1.0, Color32::from_white_alpha(18)),
    );
    ui.add_space(4.0);
}

fn show_scrollable_menu(
    ui: &mut egui::Ui,
    id_salt: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    // Les menus gagnent des lignes plus hautes et plus respirees sans toucher
    // aux espacements du dock lui-meme.
    ui.spacing_mut().item_spacing.y = 2.0;
    ui.spacing_mut().button_padding = egui::vec2(8.0, 6.0);
    ui.spacing_mut().interact_size.y = 28.0;
    egui::ScrollArea::vertical()
        .id_salt(id_salt)
        .max_height(available_popup_height(ui.ctx()))
        .show(ui, add_contents);
}

/// Zone defilante du panneau des parametres : elle ne depasse pas le panneau
/// fixe, pour que le titre reste visible et que tout reste atteignable.
fn show_scrollable_settings(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    let max_height = available_popup_height(ui.ctx()).min(ui.available_height());
    egui::ScrollArea::vertical()
        .id_salt("settings-popup-scroll")
        .max_height(max_height)
        .show(ui, add_contents);
}

/// Intitule de section du panneau des parametres : un filet discret, puis un
/// libelle en petites majuscules pour structurer la lecture.
fn settings_section_label(ui: &mut egui::Ui, label: &str) {
    ui.add_space(10.0);
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        egui::Stroke::new(1.0, Color32::from_white_alpha(16)),
    );
    ui.add_space(7.0);
    ui.label(
        egui::RichText::new(label.to_uppercase())
            .size(10.5)
            .strong()
            .color(menu_muted_color()),
    );
    ui.add_space(2.0);
}

/// Petite marque de l'application, reprise des pastilles de l'icone.
fn app_mark(ui: &mut egui::Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        egui::CornerRadius::same((size * 0.28) as u8),
        Color32::from_rgb(30, 33, 42),
    );
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same((size * 0.28) as u8),
        egui::Stroke::new(1.0, Color32::from_white_alpha(34)),
        egui::StrokeKind::Inside,
    );
    let pill = egui::Rect::from_center_size(rect.center(), egui::vec2(size * 0.74, size * 0.30));
    painter.rect_filled(
        pill,
        egui::CornerRadius::same((size * 0.15) as u8),
        Color32::from_rgb(44, 48, 60),
    );
    let colors = [
        Color32::from_rgb(95, 200, 232),
        Color32::from_rgb(110, 168, 240),
        Color32::from_rgb(155, 140, 240),
    ];
    let radius = size * 0.05;
    for (index, color) in colors.iter().enumerate() {
        let offset = (index as f32 - 1.0) * size * 0.17;
        painter.circle_filled(pill.center() + egui::vec2(offset, 0.0), radius, *color);
    }
}

fn window_preview_columns(window_count: usize, max_width: f32) -> usize {
    let max_columns = window_count.clamp(1, WINDOW_PREVIEW_MAX_COLUMNS);
    let usable_width = (max_width - 32.0).max(0.0);
    (((usable_width + 8.0) / (WINDOW_PREVIEW_CARD_WIDTH + 8.0)).floor() as usize)
        .clamp(1, max_columns)
}

fn window_preview_width_for_columns(columns: usize) -> f32 {
    let columns = columns.clamp(1, WINDOW_PREVIEW_MAX_COLUMNS);
    columns as f32 * WINDOW_PREVIEW_CARD_WIDTH + columns.saturating_sub(1) as f32 * 8.0 + 32.0
}

fn window_preview_size(window_count: usize, max_width: f32) -> egui::Vec2 {
    let columns = window_preview_columns(window_count, max_width);
    egui::vec2(
        window_preview_width_for_columns(columns).min(max_width.max(1.0)),
        window_preview_height_for_columns(window_count, columns),
    )
}

fn search_results_height(result_count: usize) -> f32 {
    let rows = result_count.max(1);
    (SEARCH_RESULTS_FRAME_PADDING
        + rows as f32 * SEARCH_RESULT_ROW_HEIGHT
        + rows.saturating_sub(1) as f32 * SEARCH_RESULTS_ROW_SPACING)
        .min(SEARCH_RESULTS_MAX_HEIGHT)
}

fn window_preview_height_for_columns(window_count: usize, columns: usize) -> f32 {
    let columns = columns.clamp(1, WINDOW_PREVIEW_MAX_COLUMNS);
    let rows = window_count.saturating_add(columns - 1) / columns;
    let row_height = WINDOW_PREVIEW_CARD_HEIGHT + WINDOW_PREVIEW_CARD_LABEL_HEIGHT;
    WINDOW_PREVIEW_FIXED_HEIGHT
        + rows as f32 * row_height
        + rows.saturating_sub(1) as f32 * WINDOW_PREVIEW_ROW_SPACING
}

fn available_preview_height(
    ctx: &egui::Context,
    dock_position: DockPosition,
    dock_height: f32,
) -> f32 {
    let available_height = monitor_work_area(ctx).height();
    let reserved_height = if dock_position.is_vertical() {
        0.0
    } else {
        dock_height + DOCK_BOTTOM_MARGIN + DOCK_EDGE_MARGIN
    };
    (available_height - reserved_height - 2.0 * POPUP_SCREEN_MARGIN).max(0.0)
}

fn show_window_preview(
    ctx: &egui::Context,
    preview: &mut WindowPreviewGroup,
    anchor: egui::Rect,
    dock_position: DockPosition,
    language: AppLanguage,
) -> WindowPreviewAction {
    let screen_rect = ctx.screen_rect();
    let popup_available_width = match dock_position {
        DockPosition::Left => (screen_rect.right() - anchor.right() - POPUP_GAP).max(1.0),
        DockPosition::Right => (anchor.left() - screen_rect.left() - POPUP_GAP).max(1.0),
        DockPosition::Top | DockPosition::Bottom => screen_rect.width(),
    };
    let popup_size = window_preview_size(preview.windows.len(), popup_available_width);
    let columns_count = window_preview_columns(preview.windows.len(), popup_available_width);
    let popup_width = popup_size.x;
    let popup_height = popup_size
        .y
        .min(available_popup_height(ctx))
        .min(screen_rect.height());
    let popup_size = egui::vec2(popup_width, popup_height);
    let position = popup_position_for_dock(dock_position, anchor, popup_size, screen_rect);
    let card_data: Vec<(String, isize)> = preview
        .windows
        .iter()
        .map(|window| (window.title.clone(), window.window))
        .collect();
    let pixels_per_point = ctx.pixels_per_point();
    let mut action = WindowPreviewAction::default();

    let area = egui::Area::new(egui::Id::new("group-window-preview"))
        .order(egui::Order::Foreground)
        .fixed_pos(position)
        .constrain(true)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(popup_width);
                ui.set_max_width(popup_width);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&preview.app_name).strong());
                    if ui.button("×").clicked() {
                        action.dismiss = true;
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(popup_height - 48.0)
                    .show(ui, |ui| {
                        ui.columns(columns_count, |columns| {
                            for (index, (title, window)) in card_data.iter().enumerate() {
                                let column = &mut columns[index % columns_count];
                                column.vertical(|ui| {
                                    ui.set_min_width(WINDOW_PREVIEW_CARD_WIDTH);
                                    let (rect, response) = ui.allocate_exact_size(
                                        egui::vec2(
                                            WINDOW_PREVIEW_CARD_WIDTH,
                                            WINDOW_PREVIEW_CARD_HEIGHT,
                                        ),
                                        egui::Sense::click(),
                                    );
                                    ui.painter().rect_filled(
                                        rect,
                                        egui::CornerRadius::same(4),
                                        Color32::from_rgb(25, 27, 32),
                                    );

                                    let update_error =
                                        preview.thumbnails[index].as_mut().and_then(|thumbnail| {
                                            thumbnail
                                                .update_destination([
                                                    (rect.left() * pixels_per_point).round() as i32,
                                                    (rect.top() * pixels_per_point).round() as i32,
                                                    (rect.right() * pixels_per_point).round()
                                                        as i32,
                                                    (rect.bottom() * pixels_per_point).round()
                                                        as i32,
                                                ])
                                                .err()
                                        });
                                    if let Some(error) = update_error {
                                        preview.thumbnail_errors[index] = Some(error);
                                        preview.thumbnails[index] = None;
                                    }
                                    if preview.thumbnails[index].is_none() {
                                        ui.painter().text(
                                            rect.center(),
                                            Align2::CENTER_CENTER,
                                            tr(language, "preview_unavailable"),
                                            egui::FontId::proportional(14.0),
                                            Color32::LIGHT_GRAY,
                                        );
                                    }
                                    ui.painter().rect_stroke(
                                        rect,
                                        egui::CornerRadius::same(4),
                                        egui::Stroke::new(1.0, Color32::from_white_alpha(48)),
                                        egui::StrokeKind::Inside,
                                    );
                                    if response.clicked() {
                                        action.activate_window = Some(*window);
                                        action.dismiss = true;
                                    }
                                    ui.label(title).on_hover_text(title);
                                    if let Some(error) = &preview.thumbnail_errors[index] {
                                        ui.label(
                                            egui::RichText::new(error)
                                                .small()
                                                .color(Color32::LIGHT_RED),
                                        );
                                    }
                                });
                            }
                        });
                    });
            });
        });

    let clicked_outside = ctx.input(|input| {
        input.pointer.any_click()
            && input
                .pointer
                .interact_pos()
                .is_some_and(|position| !area.response.rect.contains(position))
    });
    action.dismiss |= clicked_outside || ctx.input(|input| input.key_pressed(egui::Key::Escape));
    action
}

fn viewport_origin(ctx: &egui::Context) -> egui::Vec2 {
    ctx.input(|input| {
        input
            .viewport()
            .outer_rect
            .map_or(egui::Pos2::ZERO, |rect| rect.min)
            .to_vec2()
    })
}

fn cursor_position(ctx: &egui::Context) -> Option<egui::Pos2> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::POINT;
        use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

        let pixels_per_point = ctx.pixels_per_point();
        let mut point = POINT { x: 0, y: 0 };
        if unsafe { GetCursorPos(&mut point) } == 0 {
            return None;
        }
        Some(egui::pos2(
            point.x as f32 / pixels_per_point,
            point.y as f32 / pixels_per_point,
        ))
    }

    #[cfg(not(windows))]
    {
        ctx.pointer_hover_pos()
    }
}

fn monitor_work_area(ctx: &egui::Context) -> egui::Rect {
    #[cfg(windows)]
    if let Some(work_area) = windows_monitor_work_area(ctx) {
        return work_area;
    }
    egui::Rect::from_min_size(egui::Pos2::ZERO, viewport_monitor_size(ctx))
}

#[cfg(windows)]
fn windows_monitor_work_area(ctx: &egui::Context) -> Option<egui::Rect> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };

    let pixels_per_point = ctx.pixels_per_point();
    let center = ctx.input(|input| input.viewport().outer_rect.map(|rect| rect.center()))?;
    let point = POINT {
        x: (center.x * pixels_per_point).round() as i32,
        y: (center.y * pixels_per_point).round() as i32,
    };
    unsafe {
        let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        if monitor.is_null() {
            return None;
        }
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return None;
        }
        let work = info.rcWork;
        Some(egui::Rect::from_min_max(
            egui::pos2(
                work.left as f32 / pixels_per_point,
                work.top as f32 / pixels_per_point,
            ),
            egui::pos2(
                work.right as f32 / pixels_per_point,
                work.bottom as f32 / pixels_per_point,
            ),
        ))
    }
}

/// Taille ajoutee par les separateurs places entre les groupes.
fn group_separators_size(group_count: usize, spacing: f32) -> f32 {
    group_count.saturating_sub(1) as f32 * (2.0 * spacing + 1.0)
}

fn available_dock_width(ctx: &egui::Context) -> f32 {
    (monitor_work_area(ctx).width() - 2.0 * DOCK_EDGE_MARGIN).max(MIN_WINDOW_WIDTH)
}

fn viewport_monitor_size(ctx: &egui::Context) -> egui::Vec2 {
    ctx.input(|input| input.viewport().monitor_size)
        .unwrap_or(egui::vec2(
            WINDOW_WIDTH,
            WINDOW_HEIGHT + SETTINGS_MENU_EXTRA_HEIGHT,
        ))
}

fn available_dock_height(ctx: &egui::Context) -> f32 {
    (monitor_work_area(ctx).height() - 2.0 * DOCK_EDGE_MARGIN).max(MIN_WINDOW_HEIGHT)
}

fn dock_window_height_with_system_controls(
    shortcut_count: usize,
    active_app_count: usize,
    spacing: f32,
    icon_size: f32,
    show_clock: bool,
    show_weather: bool,
    show_shutdown_button: bool,
    show_search: bool,
    system_control_count: usize,
    vertical_padding: u8,
    max_height: f32,
) -> f32 {
    let icon_count = shortcut_count.saturating_add(active_app_count);
    let cell_size = icon_size + ICON_CELL_PADDING;
    let has_icon_or_search = icon_count > 0 || show_search;
    let mut height = if icon_count == 0 {
        0.0
    } else {
        icon_count as f32 * cell_size + icon_count.saturating_sub(1) as f32 * spacing
    };

    if shortcut_count > 0 && active_app_count > 0 {
        height += 2.0 * spacing + 1.0;
    }
    if show_search {
        if icon_count > 0 {
            height += 2.0 * spacing + 1.0;
        }
        height += 44.0;
    }
    if show_shutdown_button {
        if has_icon_or_search {
            height += 2.0 * spacing + 1.0;
        }
        height += cell_size;
    }
    let information_height = {
        let mut height = 0.0;
        if show_weather {
            height += WEATHER_STACK_HEIGHT;
        }
        if show_clock {
            if show_weather {
                height += spacing;
            }
            height += CLOCK_STACK_HEIGHT;
        }
        height
    };
    if information_height > 0.0 {
        if has_icon_or_search || show_shutdown_button {
            height += 2.0 * spacing + 1.0;
        }
        height += information_height;
    }
    if system_control_count > 0 {
        if has_icon_or_search || show_shutdown_button || information_height > 0.0 {
            height += 2.0 * spacing + 1.0;
        }
        height += SYSTEM_CONTROL_SIZE;
    }

    (height + 2.0 * f32::from(vertical_padding)).clamp(MIN_WINDOW_HEIGHT, max_height)
}

fn horizontal_dock_height(
    icon_count: usize,
    icon_size: f32,
    show_clock: bool,
    show_weather: bool,
    show_shutdown_button: bool,
    show_search: bool,
    system_control_count: usize,
    vertical_padding: u8,
    max_height: f32,
) -> f32 {
    let icon_or_power_height = if icon_count > 0 || show_shutdown_button {
        icon_size + ICON_CELL_PADDING
    } else {
        0.0
    };
    let content_height = icon_or_power_height
        .max(if show_search {
            SEARCH_FIELD_HEIGHT
        } else {
            0.0
        })
        .max(if show_weather { 28.0 } else { 0.0 })
        .max(if show_clock { 20.0 } else { 0.0 })
        .max(if system_control_count > 0 {
            SYSTEM_CONTROL_SIZE
        } else {
            0.0
        });

    (content_height + 2.0 * f32::from(vertical_padding) + 2.0 * DOCK_FRAME_STROKE_WIDTH)
        .min(max_height)
}

fn horizontal_dock_height_with_rows(
    icon_count: usize,
    icon_size: f32,
    show_clock: bool,
    show_weather: bool,
    show_shutdown_button: bool,
    show_search: bool,
    system_control_count: usize,
    rows: usize,
    row_spacing: f32,
    vertical_padding: u8,
    max_height: f32,
) -> f32 {
    let rows = rows.max(1);
    if rows == 1 {
        return horizontal_dock_height(
            icon_count,
            icon_size,
            show_clock,
            show_weather,
            show_shutdown_button,
            show_search,
            system_control_count,
            vertical_padding,
            max_height,
        );
    }

    let row_height = horizontal_dock_height(
        icon_count,
        icon_size,
        show_clock,
        show_weather,
        show_shutdown_button,
        show_search,
        system_control_count,
        0,
        max_height,
    );
    let row_content_height = (row_height - 2.0 * DOCK_FRAME_STROKE_WIDTH).max(0.0);
    let frame_height = 2.0 * f32::from(vertical_padding) + 2.0 * DOCK_FRAME_STROKE_WIDTH;
    (row_content_height * rows as f32 + row_spacing * rows.saturating_sub(1) as f32 + frame_height)
        .min(max_height)
}

fn wrapped_layout_row_count(
    content_width: f32,
    available_width: f32,
    largest_item_width: f32,
) -> usize {
    if content_width <= available_width {
        return 1;
    }
    let effective_width = (available_width - largest_item_width).max(1.0);
    (content_width / effective_width).ceil() as usize
}

#[cfg(test)]
fn dock_window_width_with_system_controls(
    shortcut_count: usize,
    active_app_count: usize,
    spacing: f32,
    icon_size: f32,
    show_clock: bool,
    show_weather: bool,
    show_shutdown_button: bool,
    show_search: bool,
    system_control_count: usize,
    max_width: f32,
) -> f32 {
    dock_window_required_width_with_system_controls(
        shortcut_count,
        active_app_count,
        spacing,
        icon_size,
        show_clock,
        show_weather,
        show_shutdown_button,
        show_search,
        system_control_count,
    )
    .clamp(MIN_WINDOW_WIDTH, max_width)
}

fn dock_window_required_width_with_system_controls(
    shortcut_count: usize,
    active_app_count: usize,
    spacing: f32,
    icon_size: f32,
    show_clock: bool,
    show_weather: bool,
    show_shutdown_button: bool,
    show_search: bool,
    system_control_count: usize,
) -> f32 {
    let icon_count = shortcut_count.saturating_add(active_app_count);
    let cell_size = icon_size + ICON_CELL_PADDING;
    let mut icon_row_width = if icon_count == 0 {
        0.0
    } else {
        icon_count as f32 * cell_size + icon_count.saturating_sub(1) as f32 * spacing
    };
    if shortcut_count > 0 && active_app_count > 0 {
        icon_row_width += 2.0 * spacing + 1.0;
    }
    let has_icon_or_search = icon_count > 0 || show_search;
    if show_search {
        if icon_count > 0 {
            icon_row_width += 2.0 * spacing + 1.0;
        }
        icon_row_width += SEARCH_FIELD_WIDTH;
    }
    if show_shutdown_button {
        if has_icon_or_search {
            icon_row_width += 2.0 * spacing + 1.0;
        }
        icon_row_width += cell_size;
    }

    let mut information_width = 0.0;
    if show_weather {
        information_width += WEATHER_DISPLAY_WIDTH;
    }
    if show_clock {
        information_width += CLOCK_DISPLAY_WIDTH;
    }
    if show_clock && show_weather {
        information_width += spacing;
    }
    if (has_icon_or_search || show_shutdown_button) && information_width > 0.0 {
        information_width += 2.0 * spacing + 1.0;
    }
    if system_control_count > 0 {
        if has_icon_or_search || show_shutdown_button || information_width > 0.0 {
            information_width += 2.0 * spacing + 1.0;
        }
        information_width += system_control_count as f32 * SYSTEM_CONTROL_SIZE
            + system_control_count.saturating_sub(1) as f32 * spacing;
    }

    icon_row_width + information_width + FRAME_HORIZONTAL_MARGIN
}

fn system_control_count(show_wifi: bool, show_volume: bool) -> usize {
    usize::from(show_wifi) + usize::from(show_volume)
}

#[cfg(test)]
fn dock_window_height(
    shortcut_count: usize,
    active_app_count: usize,
    spacing: f32,
    icon_size: f32,
    show_clock: bool,
    show_weather: bool,
    show_shutdown_button: bool,
    show_search: bool,
    max_height: f32,
) -> f32 {
    dock_window_height_with_system_controls(
        shortcut_count,
        active_app_count,
        spacing,
        icon_size,
        show_clock,
        show_weather,
        show_shutdown_button,
        show_search,
        0,
        DEFAULT_VERTICAL_PADDING,
        max_height,
    )
}

fn scaled_alpha(alpha: u8, opacity: u8) -> u8 {
    (u16::from(alpha) * u16::from(opacity.min(MAX_OPACITY)) / u16::from(MAX_OPACITY)) as u8
}

#[cfg(test)]
fn dock_window_width(
    shortcut_count: usize,
    active_app_count: usize,
    spacing: f32,
    icon_size: f32,
    show_clock: bool,
    show_weather: bool,
    show_shutdown_button: bool,
    show_search: bool,
    max_width: f32,
) -> f32 {
    dock_window_width_with_system_controls(
        shortcut_count,
        active_app_count,
        spacing,
        icon_size,
        show_clock,
        show_weather,
        show_shutdown_button,
        show_search,
        0,
        max_width,
    )
}

/// Sous-menu permettant de deplacer un raccourci vers un autre groupe.
/// Renvoie l'index du groupe choisi.
fn move_shortcut_menu(
    ui: &mut egui::Ui,
    language: AppLanguage,
    group_names: &[String],
    current_group: usize,
) -> Option<usize> {
    if group_names.len() <= 1 {
        return None;
    }
    let mut target = None;
    menu_submenu(ui, MENU_ICON_MOVE, tr(language, "move_to_group"), |ui| {
        show_scrollable_menu(ui, "move-to-group-menu", |ui| {
            for (index, name) in group_names.iter().enumerate() {
                if index == current_group {
                    continue;
                }
                if menu_item(ui, MENU_ICON_GROUP, name).clicked() {
                    target = Some(index);
                    ui.close_menu();
                }
            }
        });
    });
    target
}

/// Reaffiche une application masquee du dock.
fn hidden_apps_menu(
    ui: &mut egui::Ui,
    language: AppLanguage,
    hidden_apps: &[String],
    show_app: &mut Option<String>,
    show_all_apps: &mut bool,
) {
    if hidden_apps.is_empty() {
        return;
    }
    menu_submenu(
        ui,
        MENU_ICON_SHOW,
        tr(language, "hidden_applications"),
        |ui| {
            show_scrollable_menu(ui, "hidden-apps-menu", |ui| {
                for key in hidden_apps {
                    let name = display_name(Path::new(key));
                    if menu_item(
                        ui,
                        MENU_ICON_SHOW,
                        &format!("{} {name}", tr(language, "show_application")),
                    )
                    .clicked()
                    {
                        *show_app = Some(key.clone());
                        ui.close_menu();
                    }
                }
                menu_separator(ui);
                if menu_item(
                    ui,
                    MENU_ICON_RESTORE_ALL,
                    tr(language, "show_all_applications"),
                )
                .clicked()
                {
                    *show_all_apps = true;
                    ui.close_menu();
                }
            });
        },
    );
}

#[derive(Clone, Copy)]
struct SettingsState<'a> {
    icon_size: f32,
    vertical_padding: u8,
    background_opacity: u8,
    icon_opacity: u8,
    show_clock: bool,
    show_weather: bool,
    show_wifi: bool,
    show_volume: bool,
    show_shutdown_button: bool,
    show_search: bool,
    shutdown_side: DockSide,
    weather_side: DockSide,
    dock_position: DockPosition,
    background_color: [u8; 4],
    startup_enabled: bool,
    always_on_top: bool,
    weather_city: &'a str,
    language: AppLanguage,
}

#[derive(Default)]
struct SettingsChanges {
    icon_size: Option<f32>,
    vertical_padding: Option<u8>,
    background_opacity: Option<u8>,
    icon_opacity: Option<u8>,
    show_clock: Option<bool>,
    show_weather: Option<bool>,
    show_wifi: Option<bool>,
    show_volume: Option<bool>,
    show_shutdown_button: Option<bool>,
    show_search: Option<bool>,
    shutdown_side: Option<DockSide>,
    weather_side: Option<DockSide>,
    dock_position: Option<DockPosition>,
    background_color: Option<[u8; 4]>,
    weather_city: Option<String>,
    language: Option<AppLanguage>,
    startup_enabled: Option<bool>,
    always_on_top: Option<bool>,
    settings_popup_anchor: Option<egui::Rect>,
    weather_city_draft: Option<String>,
    create_group: Option<String>,
    rename_group: Option<(usize, String)>,
    delete_group: Option<usize>,
    export_config: bool,
    import_config: bool,
    quit: bool,
    dismiss_settings_popup: bool,
}

impl SettingsChanges {
    fn merge(&mut self, incoming: Self) {
        self.icon_size = incoming.icon_size.or(self.icon_size);
        self.vertical_padding = incoming.vertical_padding.or(self.vertical_padding);
        self.background_opacity = incoming.background_opacity.or(self.background_opacity);
        self.icon_opacity = incoming.icon_opacity.or(self.icon_opacity);
        self.show_clock = incoming.show_clock.or(self.show_clock);
        self.show_weather = incoming.show_weather.or(self.show_weather);
        self.show_wifi = incoming.show_wifi.or(self.show_wifi);
        self.show_volume = incoming.show_volume.or(self.show_volume);
        self.show_shutdown_button = incoming.show_shutdown_button.or(self.show_shutdown_button);
        self.show_search = incoming.show_search.or(self.show_search);
        self.shutdown_side = incoming.shutdown_side.or(self.shutdown_side);
        self.weather_side = incoming.weather_side.or(self.weather_side);
        self.dock_position = incoming.dock_position.or(self.dock_position);
        self.background_color = incoming.background_color.or(self.background_color);
        self.weather_city = incoming.weather_city.or(self.weather_city.take());
        self.language = incoming.language.or(self.language);
        self.startup_enabled = incoming.startup_enabled.or(self.startup_enabled);
        self.always_on_top = incoming.always_on_top.or(self.always_on_top);
        self.settings_popup_anchor = incoming
            .settings_popup_anchor
            .or(self.settings_popup_anchor);
        self.weather_city_draft = incoming
            .weather_city_draft
            .or(self.weather_city_draft.take());
        self.create_group = incoming.create_group.or(self.create_group.take());
        self.rename_group = incoming.rename_group.or(self.rename_group.take());
        self.delete_group = incoming.delete_group.or(self.delete_group);
        self.export_config |= incoming.export_config;
        self.import_config |= incoming.import_config;
        self.quit |= incoming.quit;
        self.dismiss_settings_popup |= incoming.dismiss_settings_popup;
    }

    fn has_changes(&self) -> bool {
        self.icon_size.is_some()
            || self.vertical_padding.is_some()
            || self.background_opacity.is_some()
            || self.icon_opacity.is_some()
            || self.show_clock.is_some()
            || self.show_weather.is_some()
            || self.show_wifi.is_some()
            || self.show_volume.is_some()
            || self.show_shutdown_button.is_some()
            || self.show_search.is_some()
            || self.shutdown_side.is_some()
            || self.weather_side.is_some()
            || self.dock_position.is_some()
            || self.background_color.is_some()
            || self.weather_city.is_some()
            || self.language.is_some()
            || self.startup_enabled.is_some()
            || self.always_on_top.is_some()
            || self.create_group.is_some()
            || self.rename_group.is_some()
            || self.delete_group.is_some()
            || self.export_config
            || self.import_config
            || self.quit
    }
}

fn add_settings_context_menu(
    response: &egui::Response,
    settings: SettingsState<'_>,
    changes: &mut SettingsChanges,
) -> bool {
    let mut settings_submenu_open = false;
    let _ = response.context_menu(|ui| {
        settings_submenu_open = settings_menu(ui, settings, changes);
    });
    response.context_menu_opened() || settings_submenu_open
}

fn settings_menu(
    ui: &mut egui::Ui,
    settings: SettingsState<'_>,
    changes: &mut SettingsChanges,
) -> bool {
    let response = menu_item(ui, MENU_ICON_SETTINGS, tr(settings.language, "settings"));
    if response.clicked() {
        changes.settings_popup_anchor = Some(response.rect);
        ui.close_menu();
    }
    menu_separator(ui);
    if menu_item(ui, MENU_ICON_QUIT, tr(settings.language, "quit")).clicked() {
        changes.quit = true;
        ui.close_menu();
    }
    response.clicked()
}

fn settings_controls(
    ui: &mut egui::Ui,
    settings: SettingsState<'_>,
    weather_city_draft: &mut String,
    group_names: &[String],
    changes: &mut SettingsChanges,
) {
    let language = settings.language;
    ui.horizontal(|ui| {
        app_mark(ui, 32.0);
        ui.add_space(6.0);
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(APP_NAME).size(17.0).strong());
            ui.label(
                egui::RichText::new(format!("v{}", display_version()))
                    .size(11.0)
                    .color(menu_muted_color()),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let close = ui
                .add(egui::Button::new(egui::RichText::new("×").size(18.0)).frame(false))
                .on_hover_text(tr(language, "close"));
            if close.clicked() {
                changes.dismiss_settings_popup = true;
            }
        });
    });
    show_scrollable_settings(ui, |ui| {
        settings_section_label(ui, tr(language, "groups"));
        group_manager(ui, language, group_names, changes);
        settings_section_label(ui, tr(language, "settings_section_appearance"));
        ui.label(tr(language, "icon_size"));
        let mut icon_size = settings.icon_size;
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Slider::new(&mut icon_size, MIN_ICON_SIZE..=MAX_ICON_SIZE)
                        .step_by(1.0)
                        .show_value(false),
                )
                .changed()
            {
                changes.icon_size = Some(icon_size);
            }
            ui.label(format!("{:.0} px", icon_size));
        });
        ui.label(tr(language, "vertical_spacing"));
        let mut vertical_padding = settings.vertical_padding;
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Slider::new(
                        &mut vertical_padding,
                        MIN_VERTICAL_PADDING..=MAX_VERTICAL_PADDING,
                    )
                    .step_by(1.0)
                    .show_value(false),
                )
                .changed()
            {
                changes.vertical_padding = Some(vertical_padding);
            }
            ui.label(format!("{vertical_padding} px"));
        });
        ui.label(tr(language, "background_opacity"));
        let mut background_opacity = settings.background_opacity;
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Slider::new(&mut background_opacity, MIN_OPACITY..=MAX_OPACITY)
                        .step_by(1.0)
                        .show_value(false),
                )
                .changed()
            {
                changes.background_opacity = Some(background_opacity);
            }
            ui.label(format!("{background_opacity}%"));
        });
        ui.label(tr(language, "icon_opacity"));
        let mut icon_opacity = settings.icon_opacity;
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Slider::new(&mut icon_opacity, MIN_OPACITY..=MAX_OPACITY)
                        .step_by(1.0)
                        .show_value(false),
                )
                .changed()
            {
                changes.icon_opacity = Some(icon_opacity);
            }
            ui.label(format!("{icon_opacity}%"));
        });
        settings_section_label(ui, tr(language, "settings_section_content"));
        let mut show_clock = settings.show_clock;
        if ui
            .checkbox(&mut show_clock, tr(language, "show_clock"))
            .changed()
        {
            changes.show_clock = Some(show_clock);
        }
        let mut show_weather = settings.show_weather;
        if ui
            .checkbox(&mut show_weather, tr(language, "show_weather"))
            .changed()
        {
            changes.show_weather = Some(show_weather);
        }
        let mut show_wifi = settings.show_wifi;
        if ui
            .checkbox(&mut show_wifi, tr(language, "show_wifi"))
            .changed()
        {
            changes.show_wifi = Some(show_wifi);
        }
        let mut show_volume = settings.show_volume;
        if ui
            .checkbox(&mut show_volume, tr(language, "show_volume"))
            .changed()
        {
            changes.show_volume = Some(show_volume);
        }
        let mut show_shutdown_button = settings.show_shutdown_button;
        if ui
            .checkbox(
                &mut show_shutdown_button,
                tr(language, "show_shutdown_button"),
            )
            .changed()
        {
            changes.show_shutdown_button = Some(show_shutdown_button);
        }
        let mut show_search = settings.show_search;
        if ui
            .checkbox(&mut show_search, tr(language, "show_search"))
            .changed()
        {
            changes.show_search = Some(show_search);
        }
        settings_section_label(ui, tr(language, "settings_section_position"));
        side_selector(
            ui,
            tr(language, "shutdown_position"),
            "shutdown-side",
            settings.shutdown_side,
            language,
            &mut changes.shutdown_side,
        );
        side_selector(
            ui,
            tr(language, "weather_position"),
            "weather-side",
            settings.weather_side,
            language,
            &mut changes.weather_side,
        );
        dock_position_selector(
            ui,
            settings.dock_position,
            language,
            &mut changes.dock_position,
        );
        ui.horizontal(|ui| {
            ui.label(tr(language, "dock_background_color"));
            let mut background_color = dock_background_color(settings.background_color);
            if ui.color_edit_button_srgba(&mut background_color).changed() {
                changes.background_color = Some([
                    background_color.r(),
                    background_color.g(),
                    background_color.b(),
                    background_color.a(),
                ]);
            }
        });
        ui.label(tr(language, "weather_city"));
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(weather_city_draft)
                    .desired_width(110.0)
                    .hint_text(settings.weather_city),
            );
            let city = weather_city_draft.trim();
            if ui
                .add_enabled(!city.is_empty(), egui::Button::new(tr(language, "apply")))
                .clicked()
                && city != settings.weather_city
            {
                changes.weather_city = Some(city.to_owned());
            }
        });
        ui.label(tr(language, "language"));
        let mut language_choice = language;
        egui::ComboBox::from_id_salt("dock-language")
            .selected_text(language_choice.native_name())
            .show_ui(ui, |ui| {
                for choice in [
                    AppLanguage::French,
                    AppLanguage::English,
                    AppLanguage::Spanish,
                ] {
                    ui.selectable_value(&mut language_choice, choice, choice.native_name());
                }
            });
        if language_choice != language {
            changes.language = Some(language_choice);
        }
        settings_section_label(ui, tr(language, "settings_section_system"));
        let mut always_on_top = settings.always_on_top;
        if ui
            .checkbox(&mut always_on_top, tr(language, "always_on_top"))
            .changed()
        {
            changes.always_on_top = Some(always_on_top);
        }
        let mut startup_enabled = settings.startup_enabled;
        if ui
            .checkbox(&mut startup_enabled, tr(language, "startup"))
            .changed()
        {
            changes.startup_enabled = Some(startup_enabled);
        }
        settings_section_label(ui, tr(language, "settings_section_backup"));
        ui.horizontal(|ui| {
            if ui.button(tr(language, "export_config")).clicked() {
                changes.export_config = true;
            }
            if ui.button(tr(language, "import_config")).clicked() {
                changes.import_config = true;
            }
        });
    });
}

/// Gestion des groupes depuis les parametres : creation, renommage et suppression.
fn group_manager(
    ui: &mut egui::Ui,
    language: AppLanguage,
    group_names: &[String],
    changes: &mut SettingsChanges,
) {
    let draft_id = egui::Id::new("group-new-name");
    let editing_id = egui::Id::new("group-rename");
    let error_id = egui::Id::new("group-error");
    let mut new_name: String = ui
        .data(|data| data.get_temp::<String>(draft_id))
        .unwrap_or_default();
    let mut editing: Option<(usize, String)> = ui
        .data(|data| data.get_temp::<Option<(usize, String)>>(editing_id))
        .flatten();
    let mut error: Option<String> = ui
        .data(|data| data.get_temp::<Option<String>>(error_id))
        .flatten();

    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut new_name)
                .desired_width(120.0)
                .hint_text(tr(language, "group_name")),
        );
        let trimmed = new_name.trim();
        if ui
            .add_enabled(
                !trimmed.is_empty(),
                egui::Button::new(menu_label(
                    MENU_ICON_NEW_GROUP,
                    tr(language, "create_group_action"),
                )),
            )
            .clicked()
        {
            if group_names
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(trimmed))
            {
                error = Some(tr(language, "group_name_exists").to_owned());
            } else {
                changes.create_group = Some(trimmed.to_owned());
                new_name.clear();
                error = None;
            }
        }
    });

    for (index, name) in group_names.iter().enumerate() {
        ui.horizontal(|ui| {
            let editing_this =
                matches!(editing.as_ref(), Some((edit_index, _)) if *edit_index == index);
            if editing_this {
                let draft = match editing.as_mut() {
                    Some((_, draft)) => draft,
                    None => return,
                };
                ui.add(egui::TextEdit::singleline(draft).desired_width(120.0));
                let trimmed = draft.trim();
                let valid = !trimmed.is_empty()
                    && !group_names.iter().enumerate().any(|(other, existing)| {
                        other != index && existing.eq_ignore_ascii_case(trimmed)
                    });
                if ui
                    .add_enabled(
                        valid,
                        egui::Button::new(tr(language, "rename_group_action")),
                    )
                    .clicked()
                {
                    changes.rename_group = Some((index, trimmed.to_owned()));
                    editing = None;
                    error = None;
                }
                if ui.button(tr(language, "cancel")).clicked() {
                    editing = None;
                }
            } else {
                ui.label(name.as_str());
                if ui
                    .small_button(menu_label(MENU_ICON_RENAME, tr(language, "rename_group")))
                    .clicked()
                {
                    editing = Some((index, name.clone()));
                    error = None;
                }
                if ui
                    .add_enabled(
                        group_names.len() > 1,
                        egui::Button::new(menu_label(
                            MENU_ICON_REMOVE,
                            tr(language, "delete_group"),
                        )),
                    )
                    .clicked()
                {
                    changes.delete_group = Some(index);
                }
            }
        });
    }

    if let Some(message) = &error {
        ui.colored_label(ui.visuals().error_fg_color, message);
    }

    ui.data_mut(|data| {
        data.insert_temp(draft_id, new_name);
        data.insert_temp(editing_id, editing);
        data.insert_temp(error_id, error);
    });
}

fn dock_position_selector(
    ui: &mut egui::Ui,
    current: DockPosition,
    language: AppLanguage,
    change: &mut Option<DockPosition>,
) {
    let mut selected = current;
    ui.horizontal(|ui| {
        ui.label(tr(language, "dock_position"));
        egui::ComboBox::from_id_salt("dock-position")
            .selected_text(tr(
                language,
                match selected {
                    DockPosition::Bottom => "bottom",
                    DockPosition::Top => "top",
                    DockPosition::Left => "left",
                    DockPosition::Right => "right",
                },
            ))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut selected, DockPosition::Bottom, tr(language, "bottom"));
                ui.selectable_value(&mut selected, DockPosition::Top, tr(language, "top"));
                ui.selectable_value(&mut selected, DockPosition::Left, tr(language, "left"));
                ui.selectable_value(&mut selected, DockPosition::Right, tr(language, "right"));
            });
    });
    if selected != current {
        *change = Some(selected);
    }
}

fn side_selector(
    ui: &mut egui::Ui,
    label: &str,
    id: &str,
    current: DockSide,
    language: AppLanguage,
    change: &mut Option<DockSide>,
) {
    let mut selected = current;
    ui.horizontal(|ui| {
        ui.label(label);
        egui::ComboBox::from_id_salt(id)
            .selected_text(tr(
                language,
                if selected == DockSide::Left {
                    "left"
                } else {
                    "right"
                },
            ))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut selected, DockSide::Left, tr(language, "left"));
                ui.selectable_value(&mut selected, DockSide::Right, tr(language, "right"));
            });
    });
    if selected != current {
        *change = Some(selected);
    }
}

fn show_power_control(
    ui: &mut egui::Ui,
    settings: SettingsState<'_>,
    changes: &mut SettingsChanges,
) -> (bool, bool) {
    let response = power_button(ui, settings.icon_size, settings.icon_opacity as f32 / 100.0)
        .on_hover_text(tr(settings.language, "power_off"));
    let menu_open = add_settings_context_menu(&response, settings, changes);
    (response.clicked(), menu_open)
}

fn show_information_controls(
    ui: &mut egui::Ui,
    settings: SettingsState<'_>,
    include_weather: bool,
    include_clock: bool,
    current_weather: Option<&weather::CurrentWeather>,
    weather_error: Option<&str>,
    current_time: Option<&str>,
    changes: &mut SettingsChanges,
) -> bool {
    let mut context_menu_open = false;
    let language = settings.language;

    if include_weather && settings.show_weather {
        if let Some(weather) = current_weather {
            let symbol = ui
                .scope(|ui| {
                    ui.multiply_opacity(settings.icon_opacity as f32 / 100.0);
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(if weather.is_day {
                                weather_symbol(weather.weather_code)
                            } else {
                                moon_phase_symbol(chrono::Utc::now().timestamp())
                            })
                            .size(22.0),
                        )
                        .sense(egui::Sense::click()),
                    )
                })
                .inner
                .on_hover_text(format!(
                    "{} — {} {}",
                    weather_description(weather.weather_code, language),
                    tr(language, "weather_at"),
                    settings.weather_city
                ));
            context_menu_open |= add_settings_context_menu(&symbol, settings, changes);
            let temperature = ui.add(
                egui::Label::new(format!("{:.0} °C", weather.temperature_celsius))
                    .sense(egui::Sense::click()),
            );
            let tooltip = weather_error.map_or_else(
                || format!("{} {}", tr(language, "weather_at"), settings.weather_city),
                |error| format!("{}: {error}", tr(language, "weather_at")),
            );
            let temperature = temperature.on_hover_text(tooltip);
            context_menu_open |= add_settings_context_menu(&temperature, settings, changes);
        } else {
            let pending = ui
                .scope(|ui| {
                    ui.multiply_opacity(settings.icon_opacity as f32 / 100.0);
                    ui.add(egui::Label::new("…").sense(egui::Sense::click()))
                })
                .inner;
            let pending = if let Some(error) = weather_error {
                pending.on_hover_text(error)
            } else {
                pending.on_hover_text(format!(
                    "{} {}",
                    tr(language, "weather_loading_at"),
                    settings.weather_city
                ))
            };
            context_menu_open |= add_settings_context_menu(&pending, settings, changes);
        }
    }

    if include_clock {
        if let Some(time) = current_time {
            let time = ui
                .add(egui::Label::new(time).sense(egui::Sense::click()))
                .on_hover_text(tr(language, "local_time"));
            context_menu_open |= add_settings_context_menu(&time, settings, changes);
        }
    }
    context_menu_open
}

fn weather_symbol(code: u8) -> &'static str {
    match code {
        0 => "\u{2600}",
        1..=2 => "\u{26C5}",
        3 => "\u{2601}",
        45 | 48 => "\u{1F32B}",
        51..=67 | 80..=82 => "\u{1F327}",
        71..=77 | 85..=86 => "\u{2744}",
        95..=99 => "\u{26C8}",
        _ => "\u{2601}",
    }
}

fn moon_phase_symbol(unix_seconds: i64) -> &'static str {
    let elapsed = (unix_seconds - KNOWN_NEW_MOON_UNIX_SECONDS)
        .rem_euclid(LUNAR_CYCLE_SECONDS.round() as i64) as f64;
    let phase = elapsed / LUNAR_CYCLE_SECONDS;
    match (phase * 8.0).round() as usize % 8 {
        0 => "\u{1F311}",
        1 => "\u{1F312}",
        2 => "\u{1F313}",
        3 => "\u{1F314}",
        4 => "\u{1F315}",
        5 => "\u{1F316}",
        6 => "\u{1F317}",
        _ => "\u{1F318}",
    }
}

fn weather_description(code: u8, language: AppLanguage) -> &'static str {
    match (language, code) {
        (AppLanguage::English, 0) => "Clear",
        (AppLanguage::English, 1) => "Mainly clear",
        (AppLanguage::English, 2) => "Partly cloudy",
        (AppLanguage::English, 3) => "Overcast",
        (AppLanguage::English, 45 | 48) => "Fog",
        (AppLanguage::English, 51..=57) => "Drizzle",
        (AppLanguage::English, 61..=67) => "Rain",
        (AppLanguage::English, 71..=77) => "Snow",
        (AppLanguage::English, 80..=82) => "Showers",
        (AppLanguage::English, 85..=86) => "Snow showers",
        (AppLanguage::English, 95..=99) => "Thunderstorm",
        (AppLanguage::English, _) => "Variable conditions",
        (AppLanguage::Spanish, 0) => "Despejado",
        (AppLanguage::Spanish, 1) => "Principalmente despejado",
        (AppLanguage::Spanish, 2) => "Parcialmente nuboso",
        (AppLanguage::Spanish, 3) => "Nublado",
        (AppLanguage::Spanish, 45 | 48) => "Niebla",
        (AppLanguage::Spanish, 51..=57) => "Llovizna",
        (AppLanguage::Spanish, 61..=67) => "Lluvia",
        (AppLanguage::Spanish, 71..=77) => "Nieve",
        (AppLanguage::Spanish, 80..=82) => "Chubascos",
        (AppLanguage::Spanish, 85..=86) => "Chubascos de nieve",
        (AppLanguage::Spanish, 95..=99) => "Tormenta",
        (AppLanguage::Spanish, _) => "Condiciones variables",
        (AppLanguage::French, 0) => "Dégagé",
        (AppLanguage::French, 1) => "Principalement dégagé",
        (AppLanguage::French, 2) => "Partiellement nuageux",
        (AppLanguage::French, 3) => "Couvert",
        (AppLanguage::French, 45 | 48) => "Brouillard",
        (AppLanguage::French, 51..=57) => "Bruine",
        (AppLanguage::French, 61..=67) => "Pluie",
        (AppLanguage::French, 71..=77) => "Neige",
        (AppLanguage::French, 80..=82) => "Averses",
        (AppLanguage::French, 85..=86) => "Averses de neige",
        (AppLanguage::French, 95..=99) => "Orage",
        (AppLanguage::French, _) => "Conditions variables",
    }
}

fn dock_separator(ui: &mut egui::Ui, vertical: bool, height: f32) {
    if vertical {
        ui.separator();
    } else {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(1.0, height), egui::Sense::hover());
        let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
        ui.painter().vline(rect.center().x, rect.y_range(), stroke);
    }
}

fn icon_button(
    ui: &mut egui::Ui,
    icon: Option<&TextureHandle>,
    base_size: f32,
    sense: egui::Sense,
    opacity: f32,
) -> egui::Response {
    let cell_size = base_size + ICON_CELL_PADDING;
    let (cell, response) = ui.allocate_exact_size(egui::vec2(cell_size, cell_size), sense);
    let hover = ui.ctx().animate_value_with_time(
        response.id,
        if response.hovered() { 1.0 } else { 0.0 },
        0.12,
    );
    let icon_size = base_size + 8.0 * hover;
    let icon_rect = egui::Rect::from_center_size(cell.center(), egui::vec2(icon_size, icon_size));
    let mut painter = ui.painter().clone();
    painter.set_opacity(opacity);

    if let Some(icon) = icon {
        painter.image(
            icon.id(),
            icon_rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        painter.rect_filled(
            icon_rect,
            egui::CornerRadius::same(10),
            Color32::from_rgb(52, 57, 68),
        );
        let center = icon_rect.center();
        painter.rect_stroke(
            egui::Rect::from_center_size(center, egui::vec2(icon_size * 0.52, icon_size * 0.42)),
            egui::CornerRadius::same(3),
            egui::Stroke::new(1.5, Color32::from_white_alpha(210)),
            egui::StrokeKind::Inside,
        );
        painter.circle_filled(
            center + egui::vec2(icon_size * 0.24, -icon_size * 0.16),
            1.5,
            Color32::from_white_alpha(210),
        );
    }

    response
}

/// Position d'insertion d'un raccourci deplace dans un groupe : l'icone la
/// plus proche du curseur est retenue, puis le cote de son milieu ou se trouve
/// le curseur donne la place. La distance prend en compte les deux axes, ce qui
/// fonctionne aussi quand le dock passe sur plusieurs lignes.
fn shortcut_insertion_index(
    group: usize,
    pointer: egui::Pos2,
    shortcut_rects: &[(usize, usize, egui::Rect)],
    vertical: bool,
) -> usize {
    let nearest = shortcut_rects
        .iter()
        .filter(|(rect_group, _, _)| *rect_group == group)
        .min_by(|(_, _, left), (_, _, right)| {
            left.center()
                .distance_sq(pointer)
                .total_cmp(&right.center().distance_sq(pointer))
        });
    let Some((_, index, rect)) = nearest else {
        return 0;
    };
    let center = if vertical {
        rect.center().y
    } else {
        rect.center().x
    };
    let position = if vertical { pointer.y } else { pointer.x };
    if position < center {
        *index
    } else {
        index + 1
    }
}

/// Trait vertical (ou horizontal) marquant la position d'insertion d'un
/// raccourci deplace.
fn shortcut_drop_indicator(
    group: usize,
    index: usize,
    shortcut_rects: &[(usize, usize, egui::Rect)],
    vertical: bool,
) -> Option<egui::Rect> {
    let group_rects: Vec<egui::Rect> = shortcut_rects
        .iter()
        .filter(|(rect_group, _, _)| *rect_group == group)
        .map(|(_, _, rect)| *rect)
        .collect();
    let (rect, at_start) = match group_rects.get(index) {
        Some(rect) => (*rect, true),
        None => (*group_rects.last()?, false),
    };
    let thickness = 3.0;
    Some(if vertical {
        let y = if at_start { rect.top() } else { rect.bottom() };
        egui::Rect::from_center_size(
            egui::pos2(rect.center().x, y),
            egui::vec2(rect.width() * 0.8, thickness),
        )
    } else {
        let x = if at_start { rect.left() } else { rect.right() };
        egui::Rect::from_center_size(
            egui::pos2(x, rect.center().y),
            egui::vec2(thickness, rect.height() * 0.8),
        )
    })
}

fn search_field(
    ui: &mut egui::Ui,
    query: &mut String,
    language: AppLanguage,
    background_color: Color32,
) -> (egui::Response, egui::Rect) {
    let field_width = ui.available_width().min(SEARCH_FIELD_WIDTH);
    let mut edit_response = None;
    let frame = egui::Frame::new()
        .fill(background_color)
        .corner_radius(egui::CornerRadius::same(20))
        .stroke(egui::Stroke::new(1.0, Color32::from_white_alpha(35)))
        .inner_margin(egui::Margin::symmetric(10, 2))
        .show(ui, |ui| {
            ui.set_width(field_width - 20.0);
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 28.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    let (icon_rect, _) =
                        ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                    let center = icon_rect.center() - egui::vec2(1.0, 1.0);
                    let stroke = egui::Stroke::new(1.8, Color32::from_gray(205));
                    ui.painter().circle_stroke(center, 6.0, stroke);
                    ui.painter().line_segment(
                        [center + egui::vec2(4.3, 4.3), center + egui::vec2(8.0, 8.0)],
                        stroke,
                    );
                    edit_response = Some(
                        ui.add(
                            egui::TextEdit::singleline(query)
                                .desired_width(field_width - 52.0)
                                .hint_text(tr(language, "search_hint"))
                                .frame(false),
                        ),
                    );
                },
            );
        });
    let rect = frame.response.rect;
    let response = edit_response.unwrap_or(frame.response);
    (response, rect)
}

fn dock_background_color([red, green, blue, alpha]: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(red, green, blue, alpha)
}

fn dock_background_color_with_opacity([red, green, blue, alpha]: [u8; 4], opacity: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(red, green, blue, scaled_alpha(alpha, opacity))
}

fn power_button(ui: &mut egui::Ui, base_size: f32, opacity: f32) -> egui::Response {
    let cell_size = base_size + ICON_CELL_PADDING;
    let (cell, response) =
        ui.allocate_exact_size(egui::vec2(cell_size, cell_size), egui::Sense::click());
    let center = cell.center() + egui::vec2(0.0, base_size * 0.06);
    let radius = base_size * 0.32;
    let gap = 0.45;
    let segments = 32;
    let points = (0..=segments)
        .map(|step| {
            let angle = -std::f32::consts::FRAC_PI_2
                + gap
                + (std::f32::consts::TAU - 2.0 * gap) * step as f32 / segments as f32;
            center + egui::vec2(angle.cos() * radius, angle.sin() * radius)
        })
        .collect();
    let color = if response.hovered() {
        Color32::WHITE
    } else {
        Color32::from_gray(220)
    };
    let stroke = egui::Stroke::new((base_size / 18.0).clamp(1.4, 3.2), color);
    let mut painter = ui.painter().clone();
    painter.set_opacity(opacity);
    painter.add(egui::Shape::line(points, stroke));
    painter.line_segment(
        [
            center + egui::vec2(0.0, -radius * 1.35),
            center + egui::vec2(0.0, -radius * 0.05),
        ],
        stroke,
    );
    response
}

fn wifi_control(ui: &mut egui::Ui, base_size: f32, opacity: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(SYSTEM_CONTROL_SIZE, SYSTEM_CONTROL_SIZE),
        egui::Sense::click(),
    );
    let center = rect.center();
    let color = if response.hovered() {
        Color32::WHITE
    } else {
        Color32::from_gray(220)
    };
    let stroke = egui::Stroke::new((base_size / 20.0).clamp(1.4, 2.4), color);
    let mut painter = ui.painter().clone();
    painter.set_opacity(opacity);
    for radius in [base_size * 0.14, base_size * 0.24, base_size * 0.34] {
        let points = (0..=16)
            .map(|step| {
                let angle = std::f32::consts::PI + std::f32::consts::PI * step as f32 / 16.0;
                center + egui::vec2(radius * angle.cos(), radius * angle.sin())
            })
            .collect();
        painter.add(egui::Shape::line(points, stroke));
    }
    painter.circle_filled(
        center + egui::vec2(0.0, base_size * 0.28),
        (base_size / 16.0).clamp(1.5, 2.5),
        color,
    );
    response.on_hover_text("Wi-Fi")
}

fn volume_control(
    ui: &mut egui::Ui,
    settings: SettingsState<'_>,
    system_volume: &mut Option<f32>,
    volume_error: Option<&str>,
    volume_change: &mut Option<f32>,
    opacity: f32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(SYSTEM_CONTROL_SIZE, SYSTEM_CONTROL_SIZE),
        egui::Sense::click(),
    );
    let center = rect.center();
    let color = if response.hovered() {
        Color32::WHITE
    } else {
        Color32::from_gray(220)
    };
    let stroke = egui::Stroke::new((settings.icon_size / 20.0).clamp(1.4, 2.4), color);
    let speaker = [
        center + egui::vec2(-settings.icon_size * 0.22, -settings.icon_size * 0.1),
        center + egui::vec2(-settings.icon_size * 0.09, -settings.icon_size * 0.1),
        center + egui::vec2(settings.icon_size * 0.08, -settings.icon_size * 0.25),
        center + egui::vec2(settings.icon_size * 0.08, settings.icon_size * 0.25),
        center + egui::vec2(-settings.icon_size * 0.09, settings.icon_size * 0.1),
        center + egui::vec2(-settings.icon_size * 0.22, settings.icon_size * 0.1),
    ];
    let mut painter = ui.painter().clone();
    painter.set_opacity(opacity);
    painter.add(egui::Shape::convex_polygon(
        speaker.to_vec(),
        color,
        egui::Stroke::NONE,
    ));
    for radius in [settings.icon_size * 0.18, settings.icon_size * 0.3] {
        let points = (0..=12)
            .map(|step| {
                let angle = -std::f32::consts::FRAC_PI_3
                    + (2.0 * std::f32::consts::FRAC_PI_3) * step as f32 / 12.0;
                center
                    + egui::vec2(
                        settings.icon_size * 0.04 + radius * angle.cos(),
                        radius * angle.sin(),
                    )
            })
            .collect();
        painter.add(egui::Shape::line(points, stroke));
    }

    let popup_id = response.id.with("system-volume-popup");
    if response.clicked() {
        ui.memory_mut(|memory| memory.toggle_popup(popup_id));
    }
    let above_or_below = if settings.dock_position == DockPosition::Top {
        egui::AboveOrBelow::Below
    } else {
        egui::AboveOrBelow::Above
    };
    egui::popup::popup_above_or_below_widget(
        ui,
        popup_id,
        &response,
        above_or_below,
        egui::popup::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            if let Some(volume) = system_volume {
                let mut next_volume = *volume;
                if ui
                    .add(
                        egui::Slider::new(&mut next_volume, 0.0..=1.0)
                            .text(tr(settings.language, "volume")),
                    )
                    .changed()
                {
                    *volume = next_volume;
                    *volume_change = Some(next_volume);
                }
            } else if let Some(error) = volume_error {
                ui.label(error);
            } else {
                ui.label(tr(settings.language, "volume"));
            }
        },
    );
    let response = response.on_hover_text(
        volume_error
            .map(|error| format!("{}: {error}", tr(settings.language, "volume_error")))
            .unwrap_or_else(|| tr(settings.language, "volume").to_owned()),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dock_width_grows_to_fit_icons_and_is_capped() {
        let one_icon = dock_window_width(
            1,
            0,
            8.0,
            DEFAULT_ICON_SIZE,
            false,
            false,
            false,
            false,
            WINDOW_WIDTH,
        );
        let several_icons = dock_window_width(
            3,
            0,
            8.0,
            DEFAULT_ICON_SIZE,
            false,
            false,
            false,
            false,
            WINDOW_WIDTH,
        );
        assert!(several_icons > one_icon);
        assert_eq!(
            dock_window_width(
                100,
                0,
                8.0,
                DEFAULT_ICON_SIZE,
                false,
                false,
                false,
                false,
                WINDOW_WIDTH
            ),
            WINDOW_WIDTH
        );
    }

    #[test]
    fn dock_width_grows_with_icon_size() {
        let small = dock_window_width(
            3,
            0,
            8.0,
            MIN_ICON_SIZE,
            false,
            false,
            false,
            false,
            WINDOW_WIDTH,
        );
        let large = dock_window_width(
            3,
            0,
            8.0,
            MAX_ICON_SIZE,
            false,
            false,
            false,
            false,
            WINDOW_WIDTH,
        );
        assert!(large > small);
    }

    #[test]
    fn window_preview_height_grows_with_additional_rows() {
        let one_row = window_preview_height_for_columns(
            WINDOW_PREVIEW_MAX_COLUMNS,
            WINDOW_PREVIEW_MAX_COLUMNS,
        );
        let two_rows = window_preview_height_for_columns(
            WINDOW_PREVIEW_MAX_COLUMNS + 1,
            WINDOW_PREVIEW_MAX_COLUMNS,
        );

        assert!(two_rows > one_row);
        assert_eq!(
            two_rows - one_row,
            WINDOW_PREVIEW_CARD_HEIGHT
                + WINDOW_PREVIEW_CARD_LABEL_HEIGHT
                + WINDOW_PREVIEW_ROW_SPACING
        );
    }

    #[test]
    fn vertical_dock_height_grows_with_items_and_is_capped() {
        let one_item = dock_window_height(
            1,
            0,
            8.0,
            DEFAULT_ICON_SIZE,
            false,
            false,
            false,
            false,
            800.0,
        );
        let several_items = dock_window_height(
            1,
            3,
            8.0,
            DEFAULT_ICON_SIZE,
            false,
            false,
            false,
            false,
            800.0,
        );
        assert!(several_items > one_item);
        assert_eq!(
            dock_window_height(
                100,
                0,
                8.0,
                DEFAULT_ICON_SIZE,
                false,
                false,
                false,
                false,
                400.0
            ),
            400.0
        );
    }

    #[test]
    fn search_results_popup_height_tracks_results_and_is_capped() {
        assert_eq!(
            search_results_height(0),
            SEARCH_RESULTS_FRAME_PADDING + SEARCH_RESULT_ROW_HEIGHT
        );
        assert!(search_results_height(3) > search_results_height(1));
        assert_eq!(
            search_results_height(SEARCH_RESULTS_MAX_COUNT),
            SEARCH_RESULTS_MAX_HEIGHT
        );
    }

    #[test]
    fn dock_width_uses_available_monitor_space() {
        let width = dock_window_width(
            18,
            0,
            8.0,
            DEFAULT_ICON_SIZE,
            false,
            false,
            false,
            false,
            1200.0,
        );
        assert!(width > WINDOW_WIDTH);
        assert!(width < 1200.0);
    }

    #[test]
    fn dock_width_includes_enabled_clock_and_weather() {
        let icons_only = dock_window_width(
            4,
            0,
            8.0,
            DEFAULT_ICON_SIZE,
            false,
            false,
            false,
            false,
            WINDOW_WIDTH,
        );
        let with_info = dock_window_width(
            4,
            0,
            8.0,
            DEFAULT_ICON_SIZE,
            true,
            true,
            false,
            false,
            WINDOW_WIDTH,
        );
        assert!(with_info > icons_only);
    }

    #[test]
    fn dock_width_includes_the_shutdown_button() {
        let icons_only = dock_window_width(
            4,
            0,
            8.0,
            DEFAULT_ICON_SIZE,
            false,
            false,
            false,
            false,
            WINDOW_WIDTH,
        );
        let with_shutdown = dock_window_width(
            4,
            0,
            8.0,
            DEFAULT_ICON_SIZE,
            false,
            false,
            true,
            false,
            WINDOW_WIDTH,
        );
        assert!(with_shutdown > icons_only);
    }

    #[test]
    fn dock_width_includes_the_search_field() {
        let icons_only = dock_window_width(
            2,
            0,
            8.0,
            DEFAULT_ICON_SIZE,
            false,
            false,
            false,
            false,
            WINDOW_WIDTH,
        );
        let with_search = dock_window_width(
            2,
            0,
            8.0,
            DEFAULT_ICON_SIZE,
            false,
            false,
            false,
            true,
            WINDOW_WIDTH,
        );
        assert!(with_search > icons_only);
    }

    #[test]
    fn dock_dimensions_include_system_controls() {
        let icons_only_width = dock_window_width_with_system_controls(
            2,
            0,
            8.0,
            DEFAULT_ICON_SIZE,
            false,
            false,
            false,
            false,
            0,
            WINDOW_WIDTH,
        );
        let controls_width = dock_window_width_with_system_controls(
            2,
            0,
            8.0,
            DEFAULT_ICON_SIZE,
            false,
            false,
            false,
            false,
            2,
            WINDOW_WIDTH,
        );
        let icons_only_height = dock_window_height_with_system_controls(
            2,
            0,
            8.0,
            DEFAULT_ICON_SIZE,
            false,
            false,
            false,
            false,
            0,
            DEFAULT_VERTICAL_PADDING,
            800.0,
        );
        let controls_height = dock_window_height_with_system_controls(
            2,
            0,
            8.0,
            DEFAULT_ICON_SIZE,
            false,
            false,
            false,
            false,
            2,
            DEFAULT_VERTICAL_PADDING,
            800.0,
        );

        assert!(controls_width > icons_only_width);
        assert!(controls_height > icons_only_height);
    }

    #[test]
    fn vertical_dock_height_includes_configured_padding() {
        let height_without_padding = dock_window_height_with_system_controls(
            2,
            0,
            8.0,
            DEFAULT_ICON_SIZE,
            false,
            false,
            false,
            false,
            0,
            0,
            800.0,
        );
        let height_with_padding = dock_window_height_with_system_controls(
            2,
            0,
            8.0,
            DEFAULT_ICON_SIZE,
            false,
            false,
            false,
            false,
            0,
            MAX_VERTICAL_PADDING,
            800.0,
        );

        assert_eq!(
            height_with_padding - height_without_padding,
            2.0 * f32::from(MAX_VERTICAL_PADDING)
        );
    }

    #[test]
    fn horizontal_dock_height_changes_padding_without_changing_content_height() {
        let height_without_padding =
            horizontal_dock_height(2, DEFAULT_ICON_SIZE, true, true, true, true, 2, 0, 800.0);
        let height_with_padding = horizontal_dock_height(
            2,
            DEFAULT_ICON_SIZE,
            true,
            true,
            true,
            true,
            2,
            MAX_VERTICAL_PADDING,
            800.0,
        );

        assert_eq!(
            height_with_padding - height_without_padding,
            2.0 * f32::from(MAX_VERTICAL_PADDING)
        );
    }

    #[test]
    fn horizontal_dock_wraps_content_when_its_width_exceeds_the_viewport() {
        assert_eq!(wrapped_layout_row_count(400.0, 400.0, 190.0), 1);
        assert_eq!(wrapped_layout_row_count(600.0, 400.0, 190.0), 3);

        let one_row = horizontal_dock_height_with_rows(
            3,
            DEFAULT_ICON_SIZE,
            false,
            false,
            false,
            false,
            0,
            1,
            8.0,
            2,
            800.0,
        );
        let multiple_rows = horizontal_dock_height_with_rows(
            3,
            DEFAULT_ICON_SIZE,
            false,
            false,
            false,
            false,
            0,
            2,
            8.0,
            2,
            800.0,
        );
        assert!(multiple_rows > one_row);
    }

    #[test]
    fn dock_position_uses_the_work_area_for_bottom_placement() {
        assert_eq!(
            dock_viewport_position(
                DockPosition::Bottom,
                900.0,
                170.0,
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1920.0, 1080.0)),
            ),
            [510.0, 902.0]
        );
    }

    #[test]
    fn dock_position_keeps_the_dock_inside_the_work_area_above_the_taskbar() {
        let work_area = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(2560.0, 1392.0));
        let [x, y] = dock_viewport_position(DockPosition::Bottom, 900.0, 170.0, work_area);
        assert_eq!([x, y], [830.0, 1214.0]);
        assert!(y + 170.0 <= work_area.bottom());
    }

    #[test]
    fn dock_anchor_follows_the_dock_side() {
        assert_eq!(
            dock_area_anchor(DockPosition::Bottom).0,
            Align2::CENTER_BOTTOM
        );
        assert_eq!(dock_area_anchor(DockPosition::Top).0, Align2::CENTER_TOP);
        assert_eq!(dock_area_anchor(DockPosition::Left).0, Align2::LEFT_CENTER);
        assert_eq!(
            dock_area_anchor(DockPosition::Right).0,
            Align2::RIGHT_CENTER
        );
    }

    #[test]
    fn popup_height_uses_monitor_instead_of_initial_viewport_height() {
        assert_eq!(popup_height_for_available_screen(836.0), 820.0);
        assert_eq!(popup_height_for_available_screen(20.0), 4.0);
    }

    #[test]
    fn settings_popup_flips_and_stays_inside_the_available_screen() {
        let safe_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 836.0));
        let popup_size = egui::vec2(400.0, 600.0);
        let anchor = egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(131.0, 27.0));
        assert_eq!(
            popup_position_for_anchor(anchor, popup_size, safe_rect),
            egui::pos2(239.0, 50.0)
        );

        let edge_anchor =
            egui::Rect::from_min_size(egui::pos2(680.0, 750.0), egui::vec2(100.0, 30.0));
        assert_eq!(
            popup_position_for_anchor(edge_anchor, popup_size, safe_rect),
            egui::pos2(272.0, 228.0)
        );
    }

    #[test]
    fn dock_positions_stay_within_common_monitor_resolutions() {
        let resolutions = [
            egui::vec2(3840.0, 2160.0),
            egui::vec2(1920.0, 1080.0),
            egui::vec2(1366.0, 768.0),
            egui::vec2(800.0, 480.0),
        ];
        let positions = [
            DockPosition::Bottom,
            DockPosition::Top,
            DockPosition::Left,
            DockPosition::Right,
        ];

        for monitor_size in resolutions {
            let work_area = egui::Rect::from_min_size(egui::Pos2::ZERO, monitor_size);
            for position in positions {
                let width = if position.is_vertical() {
                    VERTICAL_DOCK_WIDTH.min((work_area.width() - 16.0).max(MIN_WINDOW_WIDTH))
                } else {
                    (WINDOW_WIDTH).min((work_area.width() - 16.0).max(MIN_WINDOW_WIDTH))
                };
                let height = WINDOW_HEIGHT.min((work_area.height() - 16.0).max(MIN_WINDOW_HEIGHT));
                let [x, y] = dock_viewport_position(position, width, height, work_area);

                assert!(x >= work_area.left() && x + width <= work_area.right());
                assert!(y >= work_area.top() && y + height <= work_area.bottom());
            }
        }
    }

    #[test]
    fn popups_follow_the_dock_and_stay_inside_the_viewport() {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let popup_size = egui::vec2(300.0, 200.0);
        let cases = [
            (
                DockPosition::Bottom,
                egui::Rect::from_min_size(egui::pos2(300.0, 520.0), egui::vec2(200.0, 40.0)),
            ),
            (
                DockPosition::Top,
                egui::Rect::from_min_size(egui::pos2(300.0, 20.0), egui::vec2(200.0, 40.0)),
            ),
            (
                DockPosition::Left,
                egui::Rect::from_min_size(egui::pos2(20.0, 200.0), egui::vec2(40.0, 200.0)),
            ),
            (
                DockPosition::Right,
                egui::Rect::from_min_size(egui::pos2(740.0, 200.0), egui::vec2(40.0, 200.0)),
            ),
        ];

        for (dock_position, anchor) in cases {
            let position = popup_position_for_dock(dock_position, anchor, popup_size, screen);
            assert!(position.x >= screen.left());
            assert!(position.y >= screen.top());
            assert!(position.x + popup_size.x <= screen.right());
            assert!(position.y + popup_size.y <= screen.bottom());
            match dock_position {
                DockPosition::Bottom => assert!(position.y + popup_size.y <= anchor.top()),
                DockPosition::Top => assert!(position.y >= anchor.bottom()),
                DockPosition::Left => assert!(position.x >= anchor.right()),
                DockPosition::Right => assert!(position.x + popup_size.x <= anchor.left()),
            }
        }
    }

    #[test]
    fn window_preview_uses_fewer_columns_when_space_is_limited() {
        assert_eq!(window_preview_columns(5, 700.0), 3);
        assert_eq!(window_preview_columns(5, 480.0), 2);
        assert_eq!(window_preview_columns(5, 250.0), 1);
        assert!(
            window_preview_height_for_columns(3, 2)
                > window_preview_height_for_columns(3, WINDOW_PREVIEW_MAX_COLUMNS)
        );
    }

    #[test]
    fn legacy_config_defaults_to_standard_icon_size() {
        let config: DockConfig = serde_json::from_str(
            r#"{"groups":[{"name":"Général","shortcuts":[]}],"selected_group":0}"#,
        )
        .unwrap();
        assert_eq!(config.icon_size, DEFAULT_ICON_SIZE);
        assert_eq!(config.vertical_padding, DEFAULT_VERTICAL_PADDING);
        assert_eq!(config.background_opacity, DEFAULT_OPACITY);
        assert_eq!(config.icon_opacity, DEFAULT_ICON_OPACITY);
        assert!(!config.show_clock);
        assert!(!config.show_weather);
        assert!(!config.show_wifi);
        assert!(!config.show_volume);
        assert!(!config.show_shutdown_button);
        assert!(!config.show_search);
        assert_eq!(config.shutdown_side, DockSide::Right);
        assert_eq!(config.weather_side, DockSide::Right);
        assert_eq!(config.dock_position, DockPosition::Bottom);
        assert_eq!(config.background_color, default_dock_background_color());
        assert_eq!(config.language, AppLanguage::French);
        assert!(config.always_on_top);
        assert_eq!(config.weather_city, DEFAULT_WEATHER_CITY);
    }

    #[test]
    fn vertical_padding_is_clamped_to_the_slider_range() {
        let mut config: DockConfig = serde_json::from_str(
            r#"{"groups":[{"name":"Général","shortcuts":[]}],"vertical_padding":255}"#,
        )
        .unwrap();

        config.normalize();

        assert_eq!(config.vertical_padding, MAX_VERTICAL_PADDING);
    }

    #[test]
    fn opacities_are_clamped_to_the_slider_range() {
        let mut config: DockConfig = serde_json::from_str(
            r#"{"groups":[{"name":"Général","shortcuts":[]}],"opacity":0,"icon_opacity":0}"#,
        )
        .unwrap();

        config.normalize();

        assert_eq!(config.background_opacity, MIN_OPACITY);
        assert_eq!(config.icon_opacity, MIN_OPACITY);
    }

    #[test]
    fn opacity_percent_scales_color_alpha() {
        assert_eq!(scaled_alpha(u8::MAX, MIN_OPACITY), 51);
        assert_eq!(scaled_alpha(u8::MAX, MAX_OPACITY), u8::MAX);
        assert_eq!(scaled_alpha(175, MAX_OPACITY), 175);
    }

    #[test]
    fn foreground_preference_is_persisted() {
        let mut config = DockConfig::default();
        config.always_on_top = false;

        let serialized = serde_json::to_string(&config).unwrap();
        let restored: DockConfig = serde_json::from_str(&serialized).unwrap();
        assert!(!restored.always_on_top);
    }

    #[test]
    fn search_and_locale_settings_are_persisted() {
        let mut config = DockConfig::default();
        config.show_search = true;
        config.show_wifi = true;
        config.show_volume = true;
        config.shutdown_side = DockSide::Left;
        config.weather_side = DockSide::Left;
        config.dock_position = DockPosition::Right;
        config.background_color = [80, 120, 160, 200];
        config.background_opacity = 73;
        config.icon_opacity = 64;
        config.language = AppLanguage::Spanish;
        config.weather_city = "Madrid".to_owned();

        let serialized = serde_json::to_string(&config).unwrap();
        let restored: DockConfig = serde_json::from_str(&serialized).unwrap();
        assert!(restored.show_search);
        assert!(restored.show_wifi);
        assert!(restored.show_volume);
        assert_eq!(restored.shutdown_side, DockSide::Left);
        assert_eq!(restored.weather_side, DockSide::Left);
        assert_eq!(restored.dock_position, DockPosition::Right);
        assert_eq!(restored.background_color, [80, 120, 160, 200]);
        assert_eq!(restored.background_opacity, 73);
        assert_eq!(restored.icon_opacity, 64);
        assert_eq!(restored.language, AppLanguage::Spanish);
        assert_eq!(restored.weather_city, "Madrid");
    }

    #[test]
    fn search_matches_application_names_case_insensitively() {
        let app = SearchApp {
            name: "Visual Studio Code".to_owned(),
            path: PathBuf::from("Code.lnk"),
        };
        assert!(search_app_matches(&app, "  studio "));
        assert!(search_app_matches(&app, "VISUAL"));
        assert!(!search_app_matches(&app, "notepad"));
    }

    #[test]
    fn start_menu_search_includes_nested_shortcuts_only() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("dock-search-test-{unique}"));
        let nested = root.join("Utilities");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("Editor.lnk"), []).unwrap();
        fs::write(nested.join("Notes.txt"), []).unwrap();

        let mut apps = Vec::new();
        collect_start_menu_apps(&root, &mut apps).unwrap();

        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Editor");
        assert_eq!(apps[0].path, nested.join("Editor.lnk"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn setup_imports_selected_shortcut_sources_into_groups() {
        let taskbar = vec![PathBuf::from("C:\\Taskbar\\Mail.lnk")];
        let desktop = vec![PathBuf::from("C:\\Desktop\\Editor.lnk")];
        let start_menu = vec![PathBuf::from("C:\\Menu\\Reader.lnk")];

        let groups = setup_groups(
            AppLanguage::French,
            &taskbar,
            &desktop,
            &start_menu,
            true,
            true,
            false,
        );

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].name, "Barre des tâches");
        assert_eq!(groups[0].shortcuts[0].path, taskbar[0]);
        assert_eq!(groups[1].name, "Bureau");
        assert_eq!(groups[1].shortcuts[0].path, desktop[0]);
    }

    #[test]
    fn setup_falls_back_to_the_localized_default_group() {
        let groups = setup_groups(AppLanguage::English, &[], &[], &[], false, false, false);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "General");
        assert!(groups[0].shortcuts.is_empty());
    }

    #[test]
    fn setup_deduplicates_shortcuts_across_sources() {
        let shared = PathBuf::from("C:\\Shared\\Chat.lnk");
        let groups = setup_groups(
            AppLanguage::French,
            &[shared.clone()],
            &[shared.clone()],
            &[],
            true,
            true,
            false,
        );

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].shortcuts.len(), 1);
    }

    #[test]
    fn setup_selection_skips_disabled_sources_and_duplicates() {
        let taskbar = vec![PathBuf::from("task.lnk")];
        let desktop = vec![PathBuf::from("task.lnk"), PathBuf::from("desk.lnk")];
        let setup = SetupState {
            language: AppLanguage::French,
            taskbar,
            desktop,
            start_menu: vec![PathBuf::from("menu.lnk")],
            import_taskbar: false,
            import_desktop: true,
            import_start_menu: false,
        };

        assert_eq!(
            setup.selected_paths(),
            vec![PathBuf::from("task.lnk"), PathBuf::from("desk.lnk")]
        );
    }

    #[test]
    fn collect_lnk_files_only_collects_shortcuts() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("dock-setup-links-{unique}"));
        let nested = root.join("Sub");
        fs::create_dir_all(&nested).unwrap();
        fs::write(root.join("App.lnk"), []).unwrap();
        fs::write(root.join("Readme.txt"), []).unwrap();
        fs::write(nested.join("Nested.lnk"), []).unwrap();

        let mut shallow = Vec::new();
        collect_lnk_files(&root, false, &mut shallow).unwrap();
        assert_eq!(shallow.len(), 1);

        let mut deep = Vec::new();
        collect_lnk_files(&root, true, &mut deep).unwrap();
        assert_eq!(deep.len(), 2);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn setup_ignores_the_dock_windows_shortcut() {
        let dock = PathBuf::from("C:\\Menu\\Dock Windows.lnk");
        let editor = PathBuf::from("C:\\Menu\\Editor.lnk");

        assert!(is_dock_shortcut(&dock));
        assert!(!is_dock_shortcut(&editor));
        assert_eq!(
            finish_shortcut_list(vec![dock.clone(), editor.clone(), dock]),
            vec![editor]
        );
    }

    #[test]
    fn setup_summary_uses_singular_and_plural() {
        assert_eq!(
            setup_summary(AppLanguage::French, 1),
            "1 raccourci sera ajouté au dock."
        );
        assert!(setup_summary(AppLanguage::French, 3).contains("3 raccourcis"));
        assert_eq!(
            setup_summary(AppLanguage::English, 0),
            "No shortcuts will be imported for now."
        );
    }

    #[test]
    fn weather_codes_have_icons_and_descriptions() {
        assert_eq!(weather_symbol(0), "\u{2600}");
        assert_eq!(weather_description(95, AppLanguage::French), "Orage");
        assert_eq!(
            weather_description(95, AppLanguage::English),
            "Thunderstorm"
        );
        assert_eq!(weather_description(95, AppLanguage::Spanish), "Tormenta");
    }

    #[test]
    fn moon_icon_tracks_all_eight_lunar_phases() {
        let symbols = [
            "\u{1F311}",
            "\u{1F312}",
            "\u{1F313}",
            "\u{1F314}",
            "\u{1F315}",
            "\u{1F316}",
            "\u{1F317}",
            "\u{1F318}",
        ];

        for (phase, expected) in symbols.iter().enumerate() {
            let timestamp = KNOWN_NEW_MOON_UNIX_SECONDS
                + (LUNAR_CYCLE_SECONDS * phase as f64 / symbols.len() as f64).round() as i64;
            assert_eq!(moon_phase_symbol(timestamp), *expected);
        }
    }

    #[test]
    fn starts_with_one_empty_group() {
        let config = DockConfig::default();
        assert_eq!(config.groups.len(), 1);
        assert!(config.groups[0].shortcuts.is_empty());
    }

    #[test]
    fn adds_groups_with_trimmed_unique_names() {
        let mut config = DockConfig::default();
        config.add_group("  Travail  ").unwrap();
        assert_eq!(config.groups[1].name, "Travail");
        assert_eq!(config.selected_group, 1);
        assert!(config.add_group("travail").is_err());
    }

    #[test]
    fn hides_and_shows_running_applications() {
        let mut config = DockConfig::default();
        let path = PathBuf::from("C:\\Tools\\Overlay.exe");

        assert!(!config.is_app_hidden(&path));
        assert!(config.hide_app(&path));
        assert!(config.is_app_hidden(&path));
        assert!(!config.hide_app(&path));
        assert!(config.show_app("c:\\tools\\overlay.exe"));
        assert!(!config.is_app_hidden(&path));
    }

    #[test]
    fn hidden_applications_are_normalized_and_persisted() {
        let mut config: DockConfig = serde_json::from_str(
            r#"{"groups":[{"name":"Général","shortcuts":[]}],"selected_group":0,"hidden_apps":["C:\\Tools\\Overlay.exe","c:\\tools\\overlay.exe"]}"#,
        )
        .unwrap();
        config.normalize();
        assert_eq!(config.hidden_apps, vec!["c:/tools/overlay.exe"]);

        let serialized = serde_json::to_string(&config).unwrap();
        let restored: DockConfig = serde_json::from_str(&serialized).unwrap();
        assert!(restored.is_app_hidden(Path::new("C:\\Tools\\Overlay.exe")));
        let mut restored = restored;
        assert!(restored.show_all_apps());
        assert!(restored.hidden_apps.is_empty());
    }

    #[test]
    fn renames_groups_with_trimmed_unique_names() {
        let mut config = DockConfig::default();
        config.add_group("Travail").unwrap();
        config.add_group("Jeux").unwrap();

        config.rename_group(1, "  Loisirs  ").unwrap();
        assert_eq!(config.groups[1].name, "Loisirs");
        assert!(config.rename_group(0, "Loisirs").is_err());
        assert!(config.rename_group(0, "   ").is_err());
        assert_eq!(config.groups[0].name, "Général");
    }

    #[test]
    fn does_not_add_the_same_path_twice() {
        let mut config = DockConfig::default();
        assert!(config.add_shortcut(PathBuf::from("editor.lnk")));
        assert!(!config.add_shortcut(PathBuf::from("EDITOR.LNK")));
        assert_eq!(config.groups[0].shortcuts.len(), 1);
    }

    #[test]
    fn adds_active_apps_to_the_requested_group() {
        let mut config = DockConfig::default();
        config.add_group("Travail").unwrap();

        assert!(config.add_shortcut_to_group(1, PathBuf::from("editor.exe")));
        assert!(config.groups[0].shortcuts.is_empty());
        assert_eq!(
            config.groups[1].shortcuts[0].path,
            PathBuf::from("editor.exe")
        );
    }

    #[test]
    fn groups_windows_from_the_same_application() {
        let grouped = group_running_apps(vec![
            active_apps::ActiveApplication {
                path: PathBuf::from(r"C:\Apps\editor.exe"),
                title: "Projet A".to_owned(),
                window: 1,
            },
            active_apps::ActiveApplication {
                path: PathBuf::from(r"c:\apps\EDITOR.EXE"),
                title: "Projet B".to_owned(),
                window: 2,
            },
            active_apps::ActiveApplication {
                path: PathBuf::from(r"C:\Apps\terminal.exe"),
                title: "Terminal".to_owned(),
                window: 3,
            },
        ]);

        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].windows.len(), 2);
        assert_eq!(grouped[0].windows[0].window, 1);
        assert_eq!(grouped[0].windows[1].window, 2);
        assert_eq!(grouped[1].windows.len(), 1);
        assert_eq!(grouped[1].windows[0].window, 3);
    }

    #[test]
    fn removing_a_group_moves_its_shortcuts_to_the_first_group() {
        let mut config = DockConfig::default();
        config.add_group("Travail").unwrap();
        config.add_shortcut(PathBuf::from("editor.lnk"));

        config.remove_group(config.selected_group);

        assert_eq!(config.groups.len(), 1);
        assert_eq!(
            config.groups[0].shortcuts[0].path,
            PathBuf::from("editor.lnk")
        );
    }

    #[test]
    fn pinned_applications_are_not_listed_among_running_apps() {
        let config = DockConfig {
            groups: vec![ShortcutGroup {
                name: "General".to_owned(),
                shortcuts: vec![Shortcut {
                    path: PathBuf::from("C:\\Apps\\Editor.exe"),
                }],
            }],
            ..DockConfig::default()
        };
        let apps = vec![
            RunningApp {
                path: PathBuf::from("c:\\apps\\editor.exe"),
                windows: Vec::new(),
                icon: None,
            },
            RunningApp {
                path: PathBuf::from("C:\\Apps\\Chat.exe"),
                windows: Vec::new(),
                icon: None,
            },
        ];

        assert_eq!(count_visible_running_apps(&apps, &config.pinned_keys()), 1);
    }

    #[test]
    fn pinned_keys_cover_every_group() {
        let mut config = DockConfig::default();
        config.add_group("Travail").unwrap();
        config.add_shortcut_to_group(0, PathBuf::from("C:\\Apps\\Editor.exe"));
        config.add_shortcut_to_group(1, PathBuf::from("C:\\Apps\\Chat.exe"));

        let mut keys = config.pinned_keys();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                path_key(Path::new("C:\\Apps\\Chat.exe")),
                path_key(Path::new("C:\\Apps\\Editor.exe")),
            ]
        );
    }

    #[test]
    fn removing_any_group_moves_its_shortcuts_to_the_first_group() {
        let mut config = DockConfig::default();
        config.add_group("Travail").unwrap();
        config.add_shortcut_to_group(1, PathBuf::from("chat.exe"));

        assert!(config.remove_group(1));
        assert_eq!(config.groups.len(), 1);
        assert_eq!(
            config.groups[0].shortcuts[0].path,
            PathBuf::from("chat.exe")
        );
        assert!(!config.remove_group(0));
    }

    #[test]
    fn moving_a_shortcut_moves_it_to_the_target_group() {
        let mut config = DockConfig::default();
        config.add_group("Travail").unwrap();
        config.add_shortcut_to_group(0, PathBuf::from("editor.exe"));

        assert!(config.move_shortcut(0, 0, 1));
        assert!(config.groups[0].shortcuts.is_empty());
        assert_eq!(
            config.groups[1].shortcuts[0].path,
            PathBuf::from("editor.exe")
        );
    }

    #[test]
    fn moving_a_shortcut_does_not_duplicate_it_in_the_target_group() {
        let mut config = DockConfig::default();
        config.add_group("Travail").unwrap();
        config.add_shortcut_to_group(0, PathBuf::from("C:\\Tools\\Editor.exe"));
        config.add_shortcut_to_group(1, PathBuf::from("c:\\tools\\editor.exe"));

        assert!(config.move_shortcut(0, 0, 1));
        assert!(config.groups[0].shortcuts.is_empty());
        assert_eq!(config.groups[1].shortcuts.len(), 1);
    }

    #[test]
    fn moving_a_shortcut_into_its_own_group_changes_nothing() {
        let mut config = DockConfig::default();
        config.add_shortcut(PathBuf::from("editor.exe"));

        assert!(!config.move_shortcut(0, 0, 0));
        assert_eq!(config.groups[0].shortcuts.len(), 1);
    }

    #[test]
    fn moving_a_shortcut_reorders_it_within_its_group() {
        let mut config = DockConfig::default();
        for name in ["a.exe", "b.exe", "c.exe"] {
            config.add_shortcut(PathBuf::from(name));
        }

        // La premiere icone est deposee apres la deuxieme.
        assert!(config.move_shortcut_to_position(0, 0, 0, 2));
        assert_eq!(
            config.groups[0]
                .shortcuts
                .iter()
                .map(|shortcut| shortcut.path.clone())
                .collect::<Vec<_>>(),
            vec!["b.exe", "a.exe", "c.exe"]
                .into_iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        );

        // La derniere icone est deposee en tete.
        assert!(config.move_shortcut_to_position(0, 2, 0, 0));
        assert_eq!(
            config.groups[0]
                .shortcuts
                .iter()
                .map(|shortcut| shortcut.path.clone())
                .collect::<Vec<_>>(),
            vec!["c.exe", "b.exe", "a.exe"]
                .into_iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn dropping_a_shortcut_on_itself_keeps_its_position() {
        let mut config = DockConfig::default();
        config.add_shortcut(PathBuf::from("a.exe"));
        config.add_shortcut(PathBuf::from("b.exe"));

        assert!(config.move_shortcut_to_position(0, 1, 0, 1));
        assert!(config.move_shortcut_to_position(0, 1, 0, 2));
        assert_eq!(config.groups[0].shortcuts[1].path, PathBuf::from("b.exe"));
    }

    #[test]
    fn moving_a_shortcut_to_a_position_in_another_group() {
        let mut config = DockConfig::default();
        config.add_group("Travail").unwrap();
        config.add_shortcut_to_group(0, PathBuf::from("a.exe"));
        config.add_shortcut_to_group(0, PathBuf::from("b.exe"));
        config.add_shortcut_to_group(1, PathBuf::from("c.exe"));
        config.add_shortcut_to_group(1, PathBuf::from("d.exe"));

        assert!(config.move_shortcut_to_position(0, 1, 1, 0));
        assert_eq!(config.groups[0].shortcuts.len(), 1);
        assert_eq!(
            config.groups[1]
                .shortcuts
                .iter()
                .map(|shortcut| shortcut.path.clone())
                .collect::<Vec<_>>(),
            vec!["b.exe", "c.exe", "d.exe"]
                .into_iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn moving_a_shortcut_past_the_end_appends_it() {
        let mut config = DockConfig::default();
        config.add_group("Travail").unwrap();
        config.add_shortcut_to_group(0, PathBuf::from("a.exe"));
        config.add_shortcut_to_group(1, PathBuf::from("b.exe"));

        assert!(config.move_shortcut_to_position(0, 0, 1, usize::MAX));
        assert!(config.groups[0].shortcuts.is_empty());
        assert_eq!(config.groups[1].shortcuts.len(), 2);
        assert_eq!(config.groups[1].shortcuts[1].path, PathBuf::from("a.exe"));
    }

    #[test]
    fn insertion_index_follows_the_cursor_side_of_the_icons() {
        let rects = vec![
            (
                0,
                0,
                egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(10.0, 10.0)),
            ),
            (
                0,
                1,
                egui::Rect::from_min_size(egui::pos2(20.0, 0.0), egui::vec2(10.0, 10.0)),
            ),
        ];

        assert_eq!(
            shortcut_insertion_index(0, egui::pos2(1.0, 5.0), &rects, false),
            0
        );
        assert_eq!(
            shortcut_insertion_index(0, egui::pos2(11.0, 5.0), &rects, false),
            1
        );
        assert_eq!(
            shortcut_insertion_index(0, egui::pos2(25.0, 5.0), &rects, false),
            2
        );
    }

    #[test]
    fn dropping_an_application_into_a_group_pins_it_there() {
        let mut config = DockConfig::default();
        config.add_group("Travail").unwrap();

        assert!(config.add_shortcut_to_group(1, PathBuf::from("C:\\Tools\\app.exe")));
        assert!(config.groups[0].shortcuts.is_empty());
        assert_eq!(
            config.groups[1].shortcuts[0].path,
            PathBuf::from("C:\\Tools\\app.exe")
        );
    }

    fn temporary_config_dir(name: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("dock-config-{name}-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn saved_config() -> DockConfig {
        let mut config = DockConfig::default();
        config.rename_group(0, "Travail").unwrap();
        config.add_shortcut_to_group(0, PathBuf::from("C:\\Apps\\editor.exe"));
        config
    }

    #[test]
    fn configuration_survives_a_missing_main_file_through_its_backup() {
        let dir = temporary_config_dir("missing");
        let path = dir.join("config.json");
        let config = saved_config();
        write_config(&path, &config).unwrap();
        // Deux enregistrements successifs creent la copie de secours.
        write_config(&path, &config).unwrap();
        fs::remove_file(&path).unwrap();

        let restored = load_config_with_backup(&path).unwrap();

        assert_eq!(restored.groups[0].name, "Travail");
        assert_eq!(restored.groups[0].shortcuts.len(), 1);
        assert!(path.exists(), "la configuration restauree est reecrite");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn configuration_survives_a_corrupted_main_file_through_its_backup() {
        let dir = temporary_config_dir("corrupt");
        let path = dir.join("config.json");
        let config = saved_config();
        write_config(&path, &config).unwrap();
        write_config(&path, &config).unwrap();
        fs::write(&path, b"{ pas du json").unwrap();

        let restored = load_config_with_backup(&path).unwrap();

        assert_eq!(restored.groups[0].name, "Travail");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn saving_keeps_the_previous_configuration_as_a_backup() {
        let dir = temporary_config_dir("backup");
        let path = dir.join("config.json");
        write_config(&path, &saved_config()).unwrap();

        let mut updated = DockConfig::default();
        updated.rename_group(0, "Jeux").unwrap();
        write_config(&path, &updated).unwrap();

        let main = read_config(&path).unwrap().unwrap();
        assert_eq!(main.groups[0].name, "Jeux");
        let backup = read_config(&config_backup_path(&path)).unwrap().unwrap();
        assert_eq!(backup.groups[0].name, "Travail");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn display_version_hides_the_zero_patch() {
        assert_eq!(short_version("1.28.0"), "1.28");
        assert_eq!(short_version("2.0.0"), "2.0");
        assert_eq!(short_version("1.28.1"), "1.28.1");
    }

    #[test]
    fn a_shortcut_is_pinned_in_only_one_group() {
        let mut config = DockConfig::default();
        config.add_group("Travail").unwrap();

        assert!(config.add_shortcut_to_group(0, PathBuf::from("C:\\Apps\\Editor.exe")));
        assert!(!config.add_shortcut_to_group(1, PathBuf::from("c:\\apps\\editor.exe")));
        assert_eq!(config.groups[0].shortcuts.len(), 1);
        assert!(config.groups[1].shortcuts.is_empty());
    }

    #[test]
    fn normalize_removes_duplicate_shortcuts_across_groups() {
        let mut config = DockConfig::default();
        config.add_group("Travail").unwrap();
        config.add_shortcut_to_group(0, PathBuf::from("C:\\Apps\\Editor.exe"));
        // Simule une ancienne configuration avec le meme raccourci dans un
        // second groupe.
        config.groups[1].shortcuts.push(Shortcut {
            path: PathBuf::from("c:/apps/editor.exe"),
        });

        assert!(config.normalize());
        assert_eq!(config.groups[0].shortcuts.len(), 1);
        assert!(config.groups[1].shortcuts.is_empty());
        assert!(!config.normalize());
    }

    #[test]
    fn moving_a_duplicated_shortcut_keeps_a_single_copy() {
        let mut config = DockConfig::default();
        config.add_group("Travail").unwrap();
        config.add_shortcut_to_group(0, PathBuf::from("C:\\Apps\\Editor.exe"));
        config.groups[1].shortcuts.push(Shortcut {
            path: PathBuf::from("C:\\Apps\\Editor.exe"),
        });

        // Deplacer la copie du groupe 0 vers le groupe 1 ne doit pas recreer de
        // doublon : la copie deja presente dans le groupe 1 suffit.
        assert!(config.move_shortcut(0, 0, 1));
        assert!(config.groups[0].shortcuts.is_empty());
        assert_eq!(config.groups[1].shortcuts.len(), 1);
    }
}
