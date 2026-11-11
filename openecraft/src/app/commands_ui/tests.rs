use super::*;

#[test]
fn commands_in_settings_keep_the_window_contents_open() {
    let ctx = egui::Context::default();
    let mut open = true;
    for frame in 0..4 {
        ctx.run_ui(egui::RawInput { time: Some(frame as f64), ..Default::default() }, |ui| {
            let response = egui::Window::new(tr("Settings")).collapsible(false).open(&mut open).show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr("Updates"));
                    close_command_menu(ui);
                    assert!(!ui.will_parent_close());
                });
            }).unwrap();
            assert!(response.inner.is_some(), "Settings lost its contents in frame {frame}");
        }).textures_delta.clear();
        assert!(open);
    }
}

#[test]
fn commands_in_menus_still_close_the_menu() {
    let ctx = egui::Context::default();
    let mut open = true;
    let mut shown = false;
    ctx.run_ui(egui::RawInput::default(), |ui| {
        let button = ui.button(tr("Help"));
        egui::Popup::menu(&button).open_bool(&mut open).show(|ui| {
            shown = true;
            ui.horizontal(|ui| {
                close_command_menu(ui);
                assert!(ui.will_parent_close());
            });
        });
    }).textures_delta.clear();
    assert!(shown);
    assert!(!open);
}
