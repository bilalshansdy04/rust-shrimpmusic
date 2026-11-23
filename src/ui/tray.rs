use muda::{Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

const EMBEDDED_ICON_BYTES: &[u8] = include_bytes!("../../assets/logo_small.png");

pub struct TrayController {
    _tray: TrayIcon,
    show_item_id: muda::MenuId,
    play_item_id: muda::MenuId,
    exit_item_id: muda::MenuId,
}

impl TrayController {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let menu = Menu::new();

        let show_item = MenuItem::new("Buka ShrimpMusic", true, None);
        let play_item = MenuItem::new("Putar / Jeda", true, None);
        let sep = PredefinedMenuItem::separator();
        let exit_item = MenuItem::new("Keluar dari Aplikasi", true, None);

        let show_item_id = show_item.id().clone();
        let play_item_id = play_item.id().clone();
        let exit_item_id = exit_item.id().clone();

        menu.append(&show_item)?;
        menu.append(&play_item)?;
        menu.append(&sep)?;
        menu.append(&exit_item)?;

        let icon = Self::load_embedded_icon()?;

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("ShrimpMusic Player")
            .with_icon(icon)
            .build()?;

        Ok(Self {
            _tray: tray,
            show_item_id,
            play_item_id,
            exit_item_id,
        })
    }

    fn load_embedded_icon() -> Result<Icon, Box<dyn std::error::Error>> {
        let img = image::load_from_memory(EMBEDDED_ICON_BYTES)?.into_rgba8();
        let (width, height) = img.dimensions();
        let rgba = img.into_raw();
        let icon = Icon::from_rgba(rgba, width, height)?;
        Ok(icon)
    }

    pub fn poll_events(&self) -> Option<TrayAction> {
        // 1. Menu item click events
        if let Ok(event) = muda::MenuEvent::receiver().try_recv() {
            if event.id == self.show_item_id {
                return Some(TrayAction::ShowWindow);
            } else if event.id == self.play_item_id {
                return Some(TrayAction::TogglePlayPause);
            } else if event.id == self.exit_item_id {
                return Some(TrayAction::ExitApp);
            }
        }

        // 2. Tray icon mouse click events (Double click or single click to restore window)
        if let Ok(event) = TrayIconEvent::receiver().try_recv() {
            match event {
                TrayIconEvent::DoubleClick { .. } => {
                    return Some(TrayAction::ShowWindow);
                }
                TrayIconEvent::Click { button: tray_icon::MouseButton::Left, button_state: tray_icon::MouseButtonState::Up, .. } => {
                    return Some(TrayAction::ShowWindow);
                }
                _ => {}
            }
        }

        None
    }
}

pub enum TrayAction {
    ShowWindow,
    TogglePlayPause,
    ExitApp,
}
