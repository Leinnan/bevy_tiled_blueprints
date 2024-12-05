use bevy::prelude::*;

use crate::MapObject;

pub const MY_ACCENT_COLOR: Color = Color::LinearRgba(LinearRgba {
    red: 0.901,
    green: 0.4,
    blue: 0.01,
    alpha: 1.0,
});

pub struct TiledBlueprintsDebugDisplayPlugin;

impl Plugin for TiledBlueprintsDebugDisplayPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(Update, draw_objects);
    }
}

fn draw_objects(mut gizmos: Gizmos, q: Query<&GlobalTransform, With<MapObject>>) {
    for t in q.iter() {
        let t = t.translation();
        gizmos.circle_2d(Vec2::new(t.x, t.y), 10., MY_ACCENT_COLOR);
    }
}
