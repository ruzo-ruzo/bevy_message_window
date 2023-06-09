use super::*;
use bevy::render::view::Visibility::Visible;

pub fn open_window(
    mut commands: Commands,
    mut mw_query: Query<Entity, With<Current>>,
    mut ow_event: EventReader<OpenWindowEvent>,
    asset_server: Res<AssetServer>,
    setup_config: Res<SetupConfig>,
) {
    for window_config in &mut ow_event {
        let mwb = MessageWindowBundle {
            message_window: MessageWindow {
                name: window_config.window_name.clone(),
            },
            state: WindowState::Preparing,
            waitting: window_config.wait_breaker,
            script: LoadedScript {
                bms_handle: asset_server.load(window_config.script_path.clone()),
                order_list: None,
            },
            popup_type: window_config.popup,
        };
        let mw_spirte = SpriteBundle {
            texture: asset_server.load(window_config.background_path.clone()),
            transform: Transform::from_translation(window_config.position.extend(0.0)),
            ..default()
        };
        let tbb = TextBoxBundle {
            text_box: TextBox {
                name: window_config.box_name.clone(),
            },
            feeding: window_config.feeding,
            config: TypeTextConfig {
                fonts: window_config
                    .font_paths
                    .iter()
                    .map(|s| asset_server.load(s))
                    .collect(),
                text_style: TextStyle {
                    font_size: window_config.font_size,
                    color: window_config.font_color,
                    ..default()
                },
                writing: window_config.writing,
                typing_timing: window_config.typing_timing,
                layer: RenderLayers::layer(setup_config.render_layer),
                alignment: window_config.main_alignment,
            },
        };
        let tb_sprite = SpriteBundle {
            sprite: Sprite {
                anchor: Anchor::TopLeft,
                color: Color::WHITE.with_a(0.),
                custom_size: Some(window_config.main_box_size),
                ..default()
            },
            transform: Transform::from_translation(window_config.main_box_origin.extend(0.0)),
            ..default()
        };
        for entity in &mut mw_query {
            commands.entity(entity).remove::<Current>();
        }
        let layer = RenderLayers::layer(setup_config.render_layer);
        let mw = match window_config.message_window_entity {
            Some(entity) => entity,
            None => commands.spawn(mw_spirte).id(),
        };
        let additional_mw = (Hidden,);
        commands
            .entity(mw)
            .insert((mwb, layer, Current, additional_mw));
        let tb = commands.spawn((tbb, tb_sprite, layer, Current)).id();
        commands.entity(mw).add_child(tb);
    }
}

#[derive(Component, Debug)]
pub struct ScalingUp {
    pub add_per_sec: f32,
}

#[allow(clippy::type_complexity)]
pub fn window_popper(
    mut commands: Commands,
    mut mw_query: Query<
        (
            Entity,
            &mut WindowState,
            &PopupType,
            &mut Visibility,
            &mut Transform,
        ),
        (With<Current>, With<MessageWindow>),
    >,
) {
    for (ent, mut ws, pt, mut vis, mut tf) in &mut mw_query {
        if *ws == WindowState::Preparing {
            match pt {
                PopupType::Scale { sec: s } => {
                    tf.scale = Vec3::new(0., 0., 0.);
                    commands.entity(ent).insert(ScalingUp {
                        add_per_sec: 1.0 / s,
                    });
                }
            }
            *vis = Visible;
            *ws = WindowState::PoppingUp;
        }
    }
}

pub fn scaling_up(
    mut commands: Commands,
    mut mw_query: Query<(Entity, &mut Transform, &ScalingUp, &mut WindowState)>,
    time: Res<Time>,
) {
    for (ent, mut tf, ScalingUp { add_per_sec: aps }, mut ws) in &mut mw_query {
        if tf.scale.x >= 1.0 {
            tf.scale = Vec3::new(1., 1., 1.);
            *ws = WindowState::Typing;
            commands.entity(ent).remove::<ScalingUp>();
        } else {
            tf.scale.x += time.delta_seconds() * aps;
            tf.scale.y += time.delta_seconds() * aps;
        };
    }
}
