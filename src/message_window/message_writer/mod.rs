use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    render::view::{RenderLayers, Visibility},
    sprite::Anchor,
    text::TextAlignment,
};

pub mod feed_animation;
pub mod typing_animations;

use super::*;
use crate::utility::*;
use feed_animation::*;

#[derive(Component, Debug)]
pub struct MessageTextLine {
    alignment: TextAlignment,
}

#[derive(Component, Debug)]
pub struct MessageTextChar;

#[derive(Bundle)]
struct CharBundle {
    text_char: MessageTextChar,
    timer: TypingTimer,
    text2d: Text2dBundle,
    layer: RenderLayers,
    writing: WritingStyle,
}

#[derive(Bundle)]
struct LineBundle {
    line: MessageTextLine,
    sprites: SpriteBundle,
}

#[derive(Component, Clone)]
pub struct TypingTimer {
    timer: Timer,
}

#[derive(SystemParam)]
#[allow(clippy::type_complexity)]
pub struct LastTextData<'w, 's> {
    text: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static Text,
            &'static TypingTimer,
            &'static Parent,
        ),
        (With<Current>, With<MessageTextChar>),
    >,
    line: Query<
        'w,
        's,
        (Entity, &'static Transform, &'static Sprite, &'static Parent),
        (With<Current>, With<MessageTextLine>),
    >,
}

type TextBoxData<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Sprite,
        &'static TypeTextConfig,
        &'static Parent,
    ),
    (With<Current>, With<TextBox>),
>;

pub fn add_new_text(
    mut commands: Commands,
    mut window_query: Query<(Entity, &mut LoadedScript, &mut WindowState)>,
    text_box_query: TextBoxData,
    last_data: LastTextData,
    mut ps_event: EventWriter<FeedWaitingEvent>,
    fonts: Res<Assets<Font>>,
    mut pending: Local<Option<Order>>,
) {
    for (w_ent, mut script, mut ws) in &mut window_query {
        for (tb_ent, tb_spr, config, parent) in &text_box_query {
            if *ws != WindowState::Typing || w_ent != parent.get() {
                continue;
            }
            let (mut last_line_opt, mut last_text_opt, mut last_x, mut last_y, mut last_timer) =
                initialize_typing_data(&last_data, tb_ent);
            let Vec2 {
                x: max_width,
                y: max_height,
            } = tb_spr.custom_size.unwrap_or_default();
            let mut in_cr = false;
            loop {
                let next_order = get_next_order(&pending, &mut script.order_list, in_cr);
                match next_order {
                    Some(Order::Type {
                        character: new_word,
                    }) => {
                        let new_text_opt = make_new_text(
                            new_word,
                            config,
                            &mut last_x,
                            last_y,
                            &mut last_timer,
                            fonts.as_ref(),
                            max_width,
                        );
                        let (Some(new_text), Some(last_line)) = (new_text_opt, last_line_opt) else {
                            *pending = next_order;
                            in_cr = true;
                            continue;
                        };
                        let new_text_entity = commands.spawn((new_text, Current)).id();
                        if let Some(last_text) = last_text_opt {
                            commands.entity(last_text).remove::<Current>();
                        }
                        last_text_opt = Some(new_text_entity);
                        commands.entity(last_line).add_child(new_text_entity);
                        *pending = None;
                    }
                    Some(Order::CarriageReturn) => {
                        let new_line_opt =
                            make_empty_line(config, &mut last_x, &mut last_y, max_height);
                        let Some(new_line) = new_line_opt else {
                            send_feed_event(&mut ps_event, w_ent, &last_timer, &mut ws);
                            *pending = next_order;
                            break;
                        };
                        let new_line_entity = commands.spawn((new_line, Current)).id();
                        if let Some(last_line) = last_line_opt {
                            commands.entity(last_line).remove::<Current>();
                        }
                        last_line_opt = Some(new_line_entity);
                        commands.entity(tb_ent).add_child(new_line_entity);
                        in_cr = false;
                        continue;
                    }
                    Some(Order::PageFeed) => {
                        send_feed_event(&mut ps_event, w_ent, &last_timer, &mut ws);
                        break;
                    }
                    _ => break,
                }
            }
        }
    }
}

fn initialize_typing_data(
    last_data: &LastTextData,
    text_box_entity: Entity,
) -> (Option<Entity>, Option<Entity>, f32, f32, TypingTimer) {
    let last_line_data_opt = last_data.line.iter().find(|x| x.3.get() == text_box_entity);
    let last_line_opt = last_line_data_opt.map(|x| x.0);
    let last_text_data_opt = last_data
        .text
        .iter()
        .find(|x| Some(x.4.get()) == last_line_opt);
    let last_text_opt = last_text_data_opt.map(|x| x.0);
    let new_timer = TypingTimer {
        timer: Timer::from_seconds(0., TimerMode::Once),
    };
    let last_timer = last_text_data_opt
        .map(|x| (*x.3).clone())
        .unwrap_or(new_timer);
    let last_x = last_text_data_opt
        .and_then(|t| {
            t.2.sections
                .first()
                .map(|s| t.1.translation.x + s.style.font_size)
        })
        .unwrap_or_default();
    let last_y = last_line_data_opt
        .and_then(|l| l.2.custom_size.map(|c| l.1.translation.y + c.y))
        .unwrap_or_default();
    (last_line_opt, last_text_opt, last_x, last_y, last_timer)
}

fn send_feed_event(
    fw_event: &mut EventWriter<FeedWaitingEvent>,
    entity: Entity,
    last_timer: &TypingTimer,
    ws: &mut WindowState,
) {
    fw_event.send(FeedWaitingEvent {
        target_window: entity,
        wait_sec: last_timer.timer.remaining_secs(),
    });
    *ws = WindowState::Waiting;
}

fn get_next_order(
    pending: &Option<Order>,
    order_list: &mut Option<Vec<Order>>,
    in_cr: bool,
) -> Option<Order> {
    match (pending, order_list, in_cr) {
        (_, _, true) => Some(Order::CarriageReturn),
        (s @ Some(_), _, _) => s.clone(),
        (None, Some(ref mut list), _) => list.pop(),
        _ => None,
    }
}

//Todo: カーニングつける。
fn make_new_text(
    new_word: char,
    config: &TypeTextConfig,
    last_x: &mut f32,
    last_y: f32,
    last_timer: &mut TypingTimer,
    font_assets: &Assets<Font>,
    max_width: f32,
) -> Option<CharBundle> {
    let next_x = *last_x + config.text_style.font_size;
    if next_x > max_width {
        None
    } else {
        let text_style = TextStyle {
            font: choice_font(&config.fonts, new_word, font_assets).unwrap_or_default(),
            ..config.text_style
        };
        let text2d_bundle = Text2dBundle {
            text: Text::from_section(new_word.to_string(), text_style),
            transform: Transform::from_translation(Vec3::new(next_x, 0., 1.)),
            visibility: Visibility::Hidden,
            text_anchor: Anchor::BottomLeft,
            ..default()
        };
        let last_secs = last_timer.timer.remaining_secs();
        let type_sec = match config.typing_timing {
            TypingTiming::ByChar { sec: s } => last_secs + s,
            TypingTiming::ByLine { sec: s } => {
                let is_first_char = last_y >= -config.text_style.font_size;
                last_secs
                    + if *last_x == 0. && !is_first_char {
                        s
                    } else {
                        0.
                    }
            }
            _ => 0.,
        };
        let typing_timer = TypingTimer {
            timer: Timer::from_seconds(type_sec, TimerMode::Once),
        };
        *last_x += config.text_style.font_size;
        *last_timer = typing_timer.clone();
        Some(CharBundle {
            text_char: MessageTextChar,
            timer: typing_timer,
            text2d: text2d_bundle,
            layer: config.layer,
            writing: config.writing,
        })
    }
}

//Todo: 高さ調整つける
fn make_empty_line(
    config: &TypeTextConfig,
    last_x: &mut f32,
    last_y: &mut f32,
    max_height: f32,
) -> Option<LineBundle> {
    *last_x = 0.;
    *last_y -= config.text_style.font_size;
    if *last_y < -max_height {
        None
    } else {
        let sprite_bundle = SpriteBundle {
            sprite: Sprite {
                anchor: Anchor::BottomLeft,
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(0., *last_y, 0.)),
            ..default()
        };
        Some(LineBundle {
            sprites: sprite_bundle,
            line: MessageTextLine {
                alignment: config.alignment,
            },
        })
    }
}

pub fn settle_lines(
    mut targets: Query<
        (
            &MessageTextLine,
            &mut Transform,
            &mut Sprite,
            &Children,
            &Parent,
        ),
        Without<TextBox>,
    >,
    text_char: Query<&Text, With<MessageTextChar>>,
    text_box: Query<&Sprite, With<TextBox>>,
) {
    for (mtl, mut l_tf, mut sprite, children, parent) in &mut targets {
        let text_size_list: Vec<f32> = text_char
            .iter_many(children)
            .map(|c| {
                c.sections
                    .first()
                    .map(|t| t.style.font_size)
                    .unwrap_or_default()
            })
            .collect();
        let line_width: f32 = text_size_list.iter().sum();
        let line_hight = text_size_list
            .into_iter()
            .reduce(|x, y| if x > y { x } else { y })
            .unwrap_or_default();
        sprite.custom_size = Some(Vec2::new(line_width, line_hight));
        let box_width = text_box
            .get(parent.get())
            .ok()
            .and_then(|b| b.custom_size.map(|s| s.x))
            .unwrap_or_default();
        l_tf.translation.x = match mtl.alignment {
            TextAlignment::Center => (box_width - line_width) / 2.,
            TextAlignment::Right => box_width - line_width,
            _ => 0.,
        }
    }
}