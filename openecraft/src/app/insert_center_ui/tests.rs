use super::*;

fn list_frame(ctx: &egui::Context, offset: f32, count: usize) -> (Vec<(usize, egui::Id, f32)>, f32) {
    let mut rows = Vec::new();
    let mut content_height = 0.0;
    let mut output = ctx.run_ui(egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 240.0))),
        ..Default::default()
    }, |ui| {
        rows.clear();
        let output = egui::ScrollArea::vertical().vertical_scroll_offset(offset).show(ui, |ui| {
            visible_rows(ui, 26.0, count, |ui, index| {
                let response = ui.button(index.to_string());
                rows.push((index, response.id, response.rect.top()));
            });
        });
        content_height = output.content_size.y;
    });
    output.textures_delta.clear();
    (rows, content_height)
}

#[test]
fn long_lists_create_only_visible_widgets_and_keep_the_full_scroll_range() {
    let ctx = egui::Context::default();
    let (top, height) = list_frame(&ctx, 0.0, 50_000);
    assert!(!top.is_empty() && top.len() < 12, "{} widgets", top.len());
    assert_eq!(top[0].0, 0);
    assert!(height > 1_000_000.0);
    let (middle, _) = list_frame(&ctx, 100_000.0, 50_000);
    assert!(!middle.is_empty() && middle.len() < 12);
    assert!(middle[0].0 > 3_000);
    let (last, _) = list_frame(&ctx, height, 50_000);
    assert_eq!(last.last().unwrap().0, 49_999);
    assert!(last.len() < 12);
    assert!(list_frame(&ctx, 0.0, 0).0.is_empty());
}

#[test]
fn scrolling_preserves_widget_identity_and_row_pitch() {
    let ctx = egui::Context::default();
    let (before, _) = list_frame(&ctx, 0.0, 100);
    let (after, _) = list_frame(&ctx, 29.0, 100);
    let common = before.iter().find(|a| after.iter().any(|b| b.0 == a.0)).unwrap();
    let shifted = after.iter().find(|row| row.0 == common.0).unwrap();
    assert_eq!(common.1, shifted.1, "scrolling must not transfer focus to another entry");
    assert!((common.2 - shifted.2 - 29.0).abs() < 0.1);
    for pair in after.windows(2) {
        assert!((pair[1].2 - pair[0].2 - 29.0).abs() < 0.1);
    }
}
