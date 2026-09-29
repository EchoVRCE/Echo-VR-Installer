//! Headless UI snapshots: the same shots as snapshot mode (`snapshot.rs`), rendered
//! offscreen with wgpu instead of screenshotting a window, so they also work with the
//! screen locked or on a machine without a desktop session.
//!
//! `ECHOVR_SNAPSHOTS=<dir> cargo test headless -- --ignored` (`ECHOVR_SNAPSHOTS_ONLY`
//! filters as in snapshot mode; `ECHOVR_SNAPSHOTS_SCALE=2` renders at 2x;
//! `ECHOVR_SNAPSHOTS_FEED=live` shows the real SERVER INFO and news instead of made-up ones).

use super::{launcher, snapshot, theme, App};

#[test]
#[ignore = "needs a GPU; run with ECHOVR_SNAPSHOTS=<dir> cargo test headless -- --ignored"]
fn headless_snapshots() {
    let Some(dir) = std::env::var_os("ECHOVR_SNAPSHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let scale = std::env::var("ECHOVR_SNAPSHOTS_SCALE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);
    let live = std::env::var("ECHOVR_SNAPSHOTS_FEED").is_ok_and(|v| v == "live");
    for shot in snapshot::shots() {
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(launcher::W, launcher::H))
            .with_pixels_per_point(scale)
            .wgpu()
            .build_eframe(|cc| {
                theme::install_fonts(&cc.egui_ctx);
                theme::install_style(&cc.egui_ctx);
                let mut app = App::default();
                app.menu.demo = true;
                app.menu.feed_live = live;
                app.menu.page = shot.page;
                app.menu.snap_variant = shot.variant;
                app
            });
        harness.run_steps(8);
        // The live feed downloads in the background: wait for it (at most 20 s).
        let started = std::time::Instant::now();
        while live
            && !harness.state().menu.feed_settled()
            && started.elapsed() < std::time::Duration::from_secs(20)
        {
            std::thread::sleep(std::time::Duration::from_millis(100));
            harness.step();
        }
        harness.run_steps(4);
        let img = harness.render().expect("render");
        img.save(dir.join(format!("{}.png", shot.name))).unwrap();
    }
}
