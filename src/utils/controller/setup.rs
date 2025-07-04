use super::super::super::frontend::*;
use super::super::controller::*;

pub fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut writer1: EventWriter<UpdateNodesEvent>,
    mut writer2: EventWriter<MakeLinesEvent>,
    configs: Res<Configs>,
    state: Res<MainState>,
) {
    if let MainState::Sim = *state {
        commands.spawn((
            Camera2d::default(),
            Projection::from(OrthographicProjection {
                scaling_mode: ScalingMode::Fixed {
                    width: 1920.0,
                    height: 1080.0,
                },
                ..OrthographicProjection::default_2d()
            }),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ));

        let config: Config = configs.0.clone();

        for client in config.client.iter() {
            commands
                .spawn((
                    ScNode {
                        id: client.id,
                        connected_node_ids: client.connected_drone_ids.clone(),
                        node_type: components::NodeType::Client,
                        pdr: 0.0,
                    },
                    Sprite::from_image(asset_server.load("controller/Client.png")),
                    Transform::from_xyz(0.0, 0.0, 0.0),
                    Pickable::default(),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Text2d::new(format!("id: {}", client.id)),
                        Transform::from_xyz(0.0, -50.0, 1.0),
                    ));
                });
        }

        for server in config.server.iter() {
            commands
                .spawn((
                    ScNode {
                        id: server.id,
                        connected_node_ids: server.connected_drone_ids.clone(),
                        node_type: components::NodeType::Server,
                        pdr: 0.0,
                    },
                    Sprite::from_image(asset_server.load("controller/Server.png")),
                    Transform::from_xyz(0.0, 0.0, 0.0),
                    Pickable::default(),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Text2d::new(format!("id: {}", server.id)),
                        Transform::from_xyz(0.0, -50.0, 1.0),
                    ));
                });
        }

        for drone in config.drone.iter() {
            commands
                .spawn((
                    ScNode {
                        id: drone.id,
                        connected_node_ids: drone.connected_node_ids.clone(),
                        node_type: components::NodeType::Drone,
                        pdr: drone.pdr,
                    },
                    Sprite::from_image(asset_server.load("controller/Drone.png")),
                    Transform::from_xyz(0.0, 0.0, 0.0),
                    Pickable::default(),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        DroneText,
                        Text2d::new(format!("id: {}  pdr: {}", drone.id, drone.pdr)),
                        Transform::from_xyz(0.0, -50.0, 1.0),
                    ));
                });
        }

        writer1.write(UpdateNodesEvent);
        writer2.write(MakeLinesEvent);
    }
}

pub fn setup_ui(mut commands: Commands, state: Res<MainState>) {
    if let MainState::Sim = *state {
        let bar_color = Color::srgb(0.8, 0.8, 0.8);
        let button_color = Color::srgb(0.6, 0.6, 0.6);
        let button_hovered_color = Color::srgb(0.5, 0.5, 0.5);
        let button_pressed_color = Color::srgb(0.4, 0.4, 0.4);
        let text_color = Color::srgb(0.1, 0.1, 0.1);
        let input_background = Color::srgb(0.85, 0.85, 0.85);
        let border_color = Color::srgb(0.1, 0.1, 0.1);
        let warn_color = Color::srgb(1.0, 0.0, 0.0);

        commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Pickable::IGNORE,
            ))
            .with_children(|parent| {
                parent
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(UI_HEIGHT),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::vertical(Val::Percent(0.5)),
                            ..default()
                        },
                        BackgroundColor(bar_color),
                    ))
                    .with_children(|parent| {
                        parent
                            .spawn(Node {
                                width: Val::Percent(100.0),
                                //height: Val::Percent(UI_HEIGHT),
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceBetween,
                                padding: UiRect::horizontal(Val::Percent(2.0)),
                                ..default()
                            })
                            .with_children(|parent| {
                                //
                                parent
                                    .spawn((
                                        Node {
                                            width: Val::Percent(30.0),
                                            height: Val::Percent(100.0),
                                            flex_direction: FlexDirection::Column,
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::FlexStart,
                                            row_gap: Val::Px(10.0),
                                            ..default()
                                        },
                                        BackgroundColor(Color::NONE),
                                    ))
                                    .with_children(|parent| {
                                        parent.spawn((
                                            Text::new("Drone Commands"),
                                            TextColor(text_color),
                                        ));

                                        parent
                                            .spawn((
                                                Node {
                                                    flex_direction: FlexDirection::Row,
                                                    column_gap: Val::Px(10.0),
                                                    ..default()
                                                },
                                                BackgroundColor(Color::NONE),
                                            ))
                                            .with_children(|row| {
                                                let labels = [
                                                    ("Add", ButtonLabel::Add),
                                                    ("Crash", ButtonLabel::Crash),
                                                    ("Connect", ButtonLabel::Connect),
                                                    ("Set PDR", ButtonLabel::Pdr),
                                                ];
                                                for (text, label) in labels {
                                                    row.spawn((
                                                        Button,
                                                        label,
                                                        Node {
                                                            width: Val::Px(100.0),
                                                            height: Val::Px(40.0),
                                                            justify_content: JustifyContent::Center,
                                                            align_items: AlignItems::Center,
                                                            ..default()
                                                        },
                                                        BackgroundColor(button_color),
                                                        Interaction::default(),
                                                        ButtonColors {
                                                            normal: button_color,
                                                            hovered: button_hovered_color,
                                                            pressed: button_pressed_color,
                                                        },
                                                    ))
                                                    .with_children(|btn| {
                                                        btn.spawn((
                                                            Text::new(text),
                                                            TextColor(text_color),
                                                        ));
                                                    });
                                                }
                                            });
                                    });
                                parent
                                    .spawn((
                                        Node {
                                            flex_direction: FlexDirection::Row,
                                            align_items: AlignItems::Center,
                                            column_gap: Val::Px(40.0),
                                            ..default()
                                        },
                                        Visibility::Hidden,
                                        BackgroundColor(Color::NONE),
                                    ))
                                    .with_children(|row| {
                                        row.spawn((
                                            Node {
                                                flex_direction: FlexDirection::Column,
                                                align_items: AlignItems::Center,
                                                row_gap: Val::Px(6.0),
                                                ..default()
                                            },
                                            DroneAdd,
                                            BackgroundColor(Color::NONE),
                                        ))
                                        .with_children(
                                            |column| {
                                                column.spawn((
                                                    Text::new("Select Drone"),
                                                    TextColor(text_color),
                                                ));

                                                column
                                                    .spawn((
                                                        Node {
                                                            flex_direction: FlexDirection::Row,
                                                            align_items: AlignItems::Center,
                                                            column_gap: Val::Px(8.0),
                                                            ..default()
                                                        },
                                                        BackgroundColor(Color::NONE),
                                                    ))
                                                    .with_children(|row| {
                                                        row.spawn((
                                                            Button,
                                                            SelectorDirection::Left,
                                                            Node {
                                                                width: Val::Px(40.0),
                                                                height: Val::Px(40.0),
                                                                justify_content:
                                                                    JustifyContent::Center,
                                                                align_items: AlignItems::Center,
                                                                ..default()
                                                            },
                                                            BackgroundColor(button_color),
                                                            Interaction::default(),
                                                            ButtonColors {
                                                                normal: button_color,
                                                                hovered: button_hovered_color,
                                                                pressed: button_pressed_color,
                                                            },
                                                        ))
                                                        .with_children(|btn| {
                                                            btn.spawn((
                                                                Text::new("<"),
                                                                TextColor(text_color),
                                                            ));
                                                        });

                                                        row.spawn((
                                                            Node {
                                                                width: Val::Px(200.0),
                                                                height: Val::Px(40.0),
                                                                justify_content:
                                                                    JustifyContent::Center,
                                                                align_items: AlignItems::Center,
                                                                padding: UiRect::horizontal(
                                                                    Val::Px(10.0),
                                                                ),
                                                                border: UiRect::all(Val::Px(1.0)),
                                                                ..default()
                                                            },
                                                            BorderColor(Color::BLACK),
                                                            BackgroundColor(Color::srgb(
                                                                0.9, 0.9, 0.9,
                                                            )),
                                                        ))
                                                        .with_children(|center| {
                                                            center.spawn((
                                                                DroneSelector { index: 0 },
                                                                Text::new(DRONE_NAMES[0]),
                                                                TextColor(text_color),
                                                            ));
                                                        });

                                                        row.spawn((
                                                            Button,
                                                            SelectorDirection::Right,
                                                            Node {
                                                                width: Val::Px(40.0),
                                                                height: Val::Px(40.0),
                                                                justify_content:
                                                                    JustifyContent::Center,
                                                                align_items: AlignItems::Center,
                                                                ..default()
                                                            },
                                                            BackgroundColor(button_color),
                                                            Interaction::default(),
                                                            ButtonColors {
                                                                normal: button_color,
                                                                hovered: button_hovered_color,
                                                                pressed: button_pressed_color,
                                                            },
                                                        ))
                                                        .with_children(|btn| {
                                                            btn.spawn((
                                                                Text::new(">"),
                                                                TextColor(text_color),
                                                            ));
                                                        });
                                                    });
                                            },
                                        );

                                        row.spawn((
                                            Node {
                                                flex_direction: FlexDirection::Column,
                                                align_items: AlignItems::Center,
                                                row_gap: Val::Px(6.0),
                                                ..default()
                                            },
                                            TextBox,
                                            BackgroundColor(Color::NONE),
                                        ))
                                        .with_children(
                                            |column| {
                                                column.spawn((
                                                    Text::new("Drone ID"),
                                                    TextColor(text_color),
                                                    TextboxTopText,
                                                ));
                                                column.spawn((
                                                    TextInput,
                                                    TextInputInactive(true),
                                                    DroneIdInput,
                                                    Node {
                                                        width: Val::Px(140.0),
                                                        height: Val::Px(40.0),
                                                        padding: UiRect::all(Val::Px(5.0)),
                                                        border: UiRect::all(Val::Px(2.0)),
                                                        ..default()
                                                    },
                                                    BorderColor(border_color),
                                                    BackgroundColor(input_background),
                                                    TextInputValue("".to_string()),
                                                ));
                                            },
                                        );
                                    });

                                parent
                                    .spawn((
                                        Node {
                                            flex_direction: FlexDirection::Column,
                                            align_items: AlignItems::Center,
                                            row_gap: Val::Px(6.0),
                                            ..default()
                                        },
                                        Visibility::Hidden,
                                        ConfirmShown,
                                        BackgroundColor(Color::NONE),
                                    ))
                                    .with_children(|column| {
                                        column
                                            .spawn((Text::new("Confirm?"), TextColor(text_color)));

                                        column
                                            .spawn((
                                                Node {
                                                    flex_direction: FlexDirection::Row,
                                                    column_gap: Val::Px(10.0),
                                                    ..default()
                                                },
                                                BackgroundColor(Color::NONE),
                                            ))
                                            .with_children(|row| {
                                                row.spawn((
                                                    Button,
                                                    ButtonLabel::Done,
                                                    Node {
                                                        width: Val::Px(100.0),
                                                        height: Val::Px(40.0),
                                                        justify_content: JustifyContent::Center,
                                                        align_items: AlignItems::Center,
                                                        ..default()
                                                    },
                                                    BackgroundColor(button_color),
                                                    Interaction::default(),
                                                    ButtonColors {
                                                        normal: button_color,
                                                        hovered: button_hovered_color,
                                                        pressed: button_pressed_color,
                                                    },
                                                ))
                                                .with_children(|btn| {
                                                    btn.spawn((
                                                        Text::new("Done"),
                                                        TextColor(text_color),
                                                    ));
                                                });
                                            });
                                    });
                            });

                       parent
                        .spawn(( //<--------------
                            Node {
                                width: Val::Percent(100.0),
                                //height: Val::Percent(0.0),
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                padding: UiRect::vertical(Val::Percent(2.0)),
                                ..default()
                            },
                            Pickable::IGNORE, //<------------------
                        )).with_children(|parent| { //<-----------------
                        parent.spawn((
                            TextWarn,
                            Text::new(""),
                            TextColor(warn_color),
                        ));
                    });


                });
        });
    }
}
