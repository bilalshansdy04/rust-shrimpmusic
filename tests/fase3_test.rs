use shrimp_music::ui::app::{AppWindow, ThemePalette};
use slint::{ComponentHandle, SharedString};
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn test_slint_ui_full_pipeline() {
    let window = AppWindow::new().expect("Failed to initialize Slint AppWindow");

    // 1. Initial State assertions
    assert_eq!(window.get_track_title(), SharedString::from("Belum Memutar Lagu"));
    assert_eq!(window.get_is_playing(), false);
    assert_eq!(window.get_is_exclusive(), false);

    // 2. Theme verification (OLED Pitch Black vs Soft Daylight)
    let theme = window.global::<ThemePalette>();
    assert!(theme.get_is_dark(), "Default theme must be Dark (OLED Pitch Black)");
    assert_eq!(theme.get_bg_base(), slint::Color::from_rgb_u8(0, 0, 0));

    // Switch to Light Mode
    theme.set_is_dark(false);
    assert_eq!(theme.get_bg_base(), slint::Color::from_rgb_u8(0xF5, 0xF5, 0xF7));

    // Switch back to Dark Mode
    theme.set_is_dark(true);
    assert_eq!(theme.get_bg_base(), slint::Color::from_rgb_u8(0, 0, 0));

    // 3. Audio Badge & Metadata State update
    window.set_track_title(SharedString::from("Bohemian Rhapsody"));
    window.set_track_artist(SharedString::from("Queen"));
    window.set_audio_format(SharedString::from("FLAC"));
    window.set_bit_depth(24);
    window.set_sample_rate(96000);
    window.set_is_exclusive(true);

    assert_eq!(window.get_track_title(), SharedString::from("Bohemian Rhapsody"));
    assert_eq!(window.get_track_artist(), SharedString::from("Queen"));
    assert_eq!(window.get_audio_format(), SharedString::from("FLAC"));
    assert_eq!(window.get_bit_depth(), 24);
    assert_eq!(window.get_sample_rate(), 96000);
    assert_eq!(window.get_is_exclusive(), true);

    // 4. Hearing Safety Modal State
    window.set_is_safety_modal_visible(true);
    assert_eq!(window.get_is_safety_modal_visible(), true);
    window.set_is_safety_modal_visible(false);
    assert_eq!(window.get_is_safety_modal_visible(), false);

    // 5. Callbacks triggering
    let play_pause_invoked = Rc::new(Cell::new(false));
    let p_clone = play_pause_invoked.clone();
    window.on_on_play_pause(move || {
        p_clone.set(true);
    });

    let seek_target = Rc::new(Cell::new(0));
    let s_clone = seek_target.clone();
    window.on_on_seek_to(move |ms| {
        s_clone.set(ms);
    });

    window.invoke_on_play_pause();
    assert!(play_pause_invoked.get());

    let seek_called = Rc::new(Cell::new(false));
    let sc = seek_called.clone();
    window.on_on_seek_to(move |ms| {
        println!("TEST on_seek_to called with ms={}", ms);
        sc.set(true);
    });

    window.set_duration_ms(180_000);
    window.set_position_ms(20_000);

    // Try sending mouse down across y from 350 to 500
    for y in (350..520).step_by(10) {
        window.window().dispatch_event(slint::platform::WindowEvent::PointerPressed {
            position: slint::LogicalPosition::new(150.0, y as f32),
            button: slint::platform::PointerEventButton::Left,
        });
        window.window().dispatch_event(slint::platform::WindowEvent::PointerReleased {
            position: slint::LogicalPosition::new(150.0, y as f32),
            button: slint::platform::PointerEventButton::Left,
        });
    }

    println!("Was seek called by mouse click? {}", seek_called.get());

    // Test dragging
    let drag_positions = Rc::new(std::cell::RefCell::new(Vec::new()));
    let dp_clone = drag_positions.clone();
    window.on_on_seek_to(move |ms| {
        println!("DRAG on_seek_to called with ms={}", ms);
        dp_clone.borrow_mut().push(ms);
    });

    // Press at x=50, y=430
    window.window().dispatch_event(slint::platform::WindowEvent::PointerPressed {
        position: slint::LogicalPosition::new(50.0, 430.0),
        button: slint::platform::PointerEventButton::Left,
    });
    // Drag to x=100, 150, 200, 250
    for x in [100.0, 150.0, 200.0, 250.0] {
        window.window().dispatch_event(slint::platform::WindowEvent::PointerMoved {
            position: slint::LogicalPosition::new(x, 430.0),
        });
    }
    // Release
    window.window().dispatch_event(slint::platform::WindowEvent::PointerReleased {
        position: slint::LogicalPosition::new(250.0, 430.0),
        button: slint::platform::PointerEventButton::Left,
    });

    println!("Total drag positions received: {}", drag_positions.borrow().len());
    for p in drag_positions.borrow().iter() {
        println!("  p: {}", p);
    }
}
