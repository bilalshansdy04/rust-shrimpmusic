use std::cell::RefCell;
use std::rc::Rc;
use slint::{Model, ModelRc, SharedString, VecModel};
use shrimp_music::ui::AppWindow;
use shrimp_music::ui::app::TrackItemModel;

#[test]
fn test_e2e_slint_tab_navigation_and_transport() {
    // 1. Initialize headless testing backend without OS window/event loop
    i_slint_backend_testing::init_no_event_loop();

    // 2. Instantiate main AppWindow component
    let app = AppWindow::new().expect("Failed to create AppWindow in headless testing environment");

    // Initial default view check (0: Library)
    assert_eq!(app.get_active_view(), 0, "Default active view should be Library (0)");

    // 3. Tab switching test (Pustaka -> Playlist -> Lirik -> Antrean)
    app.set_active_view(1);
    assert_eq!(app.get_active_view(), 1, "Active view should transition to Playlist (1)");

    app.set_active_view(2);
    assert_eq!(app.get_active_view(), 2, "Active view should transition to Lyrics (2)");

    app.set_active_view(3);
    assert_eq!(app.get_active_view(), 3, "Active view should transition to Queue (3)");

    app.set_active_view(0);
    assert_eq!(app.get_active_view(), 0, "Active view should return to Library (0)");

    // 4. Transport controls & properties verification
    assert!(!app.get_is_playing());
    app.set_is_playing(true);
    assert!(app.get_is_playing(), "is_playing should be true");

    app.set_volume(0.85);
    assert!((app.get_volume() - 0.85).abs() < 1e-4, "Volume should match set value");

    app.set_track_title(SharedString::from("Test Track Title"));
    app.set_track_artist(SharedString::from("Audiophile Artist"));
    app.set_audio_format(SharedString::from("FLAC"));
    app.set_bit_depth(24);
    app.set_sample_rate(96000);

    assert_eq!(app.get_track_title(), "Test Track Title");
    assert_eq!(app.get_track_artist(), "Audiophile Artist");
    assert_eq!(app.get_audio_format(), "FLAC");
    assert_eq!(app.get_bit_depth(), 24);
    assert_eq!(app.get_sample_rate(), 96000);

    // 5. Seek & Position verification
    app.set_position_ms(34000);
    app.set_duration_ms(180000);
    assert_eq!(app.get_position_ms(), 34000);
    assert_eq!(app.get_duration_ms(), 180000);

    // 6. Callback trigger verification (simulate user clicking play/pause and seek)
    let play_pause_invoked = Rc::new(RefCell::new(false));
    let p_clone = play_pause_invoked.clone();
    app.on_on_play_pause(move || {
        *p_clone.borrow_mut() = true;
    });

    app.invoke_on_play_pause();
    assert!(*play_pause_invoked.borrow(), "on_play_pause callback should be triggered");

    let seek_target = Rc::new(RefCell::new(0));
    let s_clone = seek_target.clone();
    app.on_on_seek_to(move |ms| {
        *s_clone.borrow_mut() = ms;
    });

    app.invoke_on_seek_to(104000);
    assert_eq!(*seek_target.borrow(), 104000, "on_seek_to callback should receive 104000 ms");
}

#[test]
fn test_e2e_slint_spotify_filter_and_playlist_search() {
    i_slint_backend_testing::init_no_event_loop();
    let app = AppWindow::new().expect("Failed to create AppWindow");

    // 1. Initial sorting state
    assert_eq!(app.get_library_sort_criteria(), 0, "Default library sort is Title (0)");
    assert!(app.get_library_sort_ascending(), "Default library sort is Ascending");

    // 2. Change sort callback test
    let received_sort = Rc::new(RefCell::new((-1, false)));
    let sort_clone = received_sort.clone();
    app.on_on_sort_changed(move |crit, asc| {
        *sort_clone.borrow_mut() = (crit, asc);
    });

    // Simulate selecting Artist (1) descending
    app.invoke_on_sort_changed(1, false);
    assert_eq!(*received_sort.borrow(), (1, false), "Sort callback should receive Artist descending");

    // 3. Playlist view sort & search
    app.set_active_view(1); // Playlist view
    app.set_selected_playlist_id(42);
    app.set_selected_playlist_name(SharedString::from("My Hi-Res Favorites"));

    assert_eq!(app.get_selected_playlist_id(), 42);
    assert_eq!(app.get_selected_playlist_name(), "My Hi-Res Favorites");

    // Verify playlist sort properties
    app.set_playlist_sort_criteria(2); // Artist
    app.set_playlist_sort_ascending(true);
    assert_eq!(app.get_playlist_sort_criteria(), 2);
    assert!(app.get_playlist_sort_ascending());

    let pl_sort_received = Rc::new(RefCell::new((-1, false)));
    let pl_sort_clone = pl_sort_received.clone();
    app.on_on_playlist_sort_changed(move |crit, asc| {
        *pl_sort_clone.borrow_mut() = (crit, asc);
    });

    app.invoke_on_playlist_sort_changed(4, false); // Recently added descending
    assert_eq!(*pl_sort_received.borrow(), (4, false));

    // 4. Playlist search query test
    let pl_search_received = Rc::new(RefCell::new(String::new()));
    let pl_search_clone = pl_search_received.clone();
    app.on_on_playlist_search_changed(move |q| {
        *pl_search_clone.borrow_mut() = q.to_string();
    });

    app.set_playlist_search_query(SharedString::from("aespa"));
    app.invoke_on_playlist_search_changed(SharedString::from("aespa"));
    assert_eq!(*pl_search_received.borrow(), "aespa");
}

#[test]
fn test_e2e_slint_track_model_and_selection() {
    i_slint_backend_testing::init_no_event_loop();
    let app = AppWindow::new().expect("Failed to create AppWindow");

    // Populate mock library tracks
    let tracks = vec![
        TrackItemModel {
            id: 1,
            title: SharedString::from("Camouflage"),
            artist: SharedString::from("aespa"),
            album: SharedString::from("LEMONADE"),
            duration_str: SharedString::from("3:07"),
            format: SharedString::from("FLAC"),
            is_hi_res: true,
            is_active: true,
            is_selected: false,
        },
        TrackItemModel {
            id: 2,
            title: SharedString::from("KISS N TELL"),
            artist: SharedString::from("aespa"),
            album: SharedString::from("KISS N TELL"),
            duration_str: SharedString::from("2:46"),
            format: SharedString::from("FLAC"),
            is_hi_res: true,
            is_active: true,
            is_selected: false,
        },
    ];

    app.set_library_tracks(ModelRc::new(VecModel::from(tracks)));
    assert_eq!(app.get_library_tracks().row_count(), 2);

    // Test double click callback to trigger isolated folder playback
    let double_clicked_track_id = Rc::new(RefCell::new(-1));
    let dc_clone = double_clicked_track_id.clone();
    app.on_on_track_double_clicked(move |id| {
        *dc_clone.borrow_mut() = id;
    });

    app.invoke_on_track_double_clicked(2);
    assert_eq!(*double_clicked_track_id.borrow(), 2, "Double click on track 2 should trigger playback callback");

    // Test selection mode callbacks
    let selected_toggle_id = Rc::new(RefCell::new(-1));
    let sel_clone = selected_toggle_id.clone();
    app.on_on_toggle_select_track(move |id| {
        *sel_clone.borrow_mut() = id;
    });

    app.invoke_on_toggle_select_track(1);
    assert_eq!(*selected_toggle_id.borrow(), 1);

    let cleared_selection = Rc::new(RefCell::new(false));
    let cl_clone = cleared_selection.clone();
    app.on_on_clear_selection(move || {
        *cl_clone.borrow_mut() = true;
    });

    app.invoke_on_clear_selection();
    assert!(*cleared_selection.borrow());
}
