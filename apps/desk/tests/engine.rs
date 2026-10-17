use kortex_desk::engine::{Layout, Rect, Workspace};
#[test]
fn focus_and_minimize_preserve_window_identity_and_geometry() {
    let mut s = Workspace::default();
    let a = s.open("code.json-formatter").expect("open");
    let b = s.open("secure.hash-calculator").expect("open");
    let before = s.windows.iter().map(|w| w.id).collect::<Vec<_>>();
    s.minimize(a);
    s.focus(a);
    assert_eq!(before, s.windows.iter().map(|w| w.id).collect::<Vec<_>>());
    assert!(!s.windows[0].minimized);
    assert!(s.windows[0].z > s.windows[1].z);
    let rect = s.windows[0].rect;
    s.maximize(a);
    s.maximize(a);
    assert_eq!(s.windows[0].rect, rect);
    s.close(b);
    assert_eq!(s.windows.len(), 1);
}
#[test]
fn layouts_are_bounded_and_spaces_restore_only_metadata() {
    let mut s = Workspace::default();
    for _ in 0..12 {
        s.open("calc.percentage").expect("window");
    }
    assert!(s.open("calc.percentage").is_none());
    for layout in [
        Layout::Halves,
        Layout::Thirds,
        Layout::Quarters,
        Layout::Grid,
        Layout::Cascade,
    ] {
        s.arrange(layout);
        for w in &s.windows {
            assert!(w.rect.x >= 0. && w.rect.y >= 0.);
            assert!(w.rect.x + w.rect.width <= 100.001);
            assert!(w.rect.y + w.rect.height <= 100.001);
        }
    }
    assert!(s.add_space());
    let json = serde_json::to_string(&s).expect("metadata");
    let mut restored: Workspace = serde_json::from_str(&json).expect("restore");
    restored.reconcile(&["calc.percentage".into()]);
    assert_eq!(restored.windows.len(), 12);
    restored.reconcile(&[]);
    assert!(restored.windows.is_empty());
    assert_eq!(restored.spaces.len(), 2);
}
#[test]
fn corrupt_geometry_and_schema_do_not_escape_canvas() {
    let r = Rect {
        x: f64::NAN,
        y: -900.,
        width: 1000.,
        height: -10.,
    }
    .bounded();
    assert_eq!(r.x, 0.);
    assert_eq!(r.y, 0.);
    assert_eq!(r.width, 100.);
    assert!(r.height > 0.);
    let mut s = Workspace {
        schema: 77,
        ..Workspace::default()
    };
    s.reconcile(&[]);
    assert_eq!(s.schema, 1);
}
