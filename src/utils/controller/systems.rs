use std::collections::HashSet;
use bevy::transform;
use super::super::super::frontend::MainState;
use super::super::controller::*;
use rand::Rng;

pub fn spawn_drone(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut node_query: Query<(Entity, &mut ScNode)>,
    mut warn_query: Query<&mut Text, With<TextWarn>>,
    mut reader: EventReader<SpawnDroneEvent>,
    mut nodes_writer: EventWriter<UpdateNodesEvent>,
    mut lines_writer: EventWriter<MakeLinesEvent>,
    mut simulation_controller: ResMut<SimulationController>,
    selector_query: Query<&DroneSelector>,
    state : Res<MainState>,
) {
    if let MainState::Sim = *state {
        for SpawnDroneEvent { id, connect_to } in reader.read() {
            
            if let Some((_, mut node)) = node_query.iter_mut().filter(|(e, _)| *e == *connect_to).next() {
                if node.node_type == components::NodeType::Client && node.connected_node_ids.len() >= 2 {
                    let mut warn_text = warn_query.single_mut().unwrap();
                    *warn_text = Text::from("Client can have at most 2 drones connected");
                }
                else{
                    let selector = selector_query.single().unwrap();
                    spawn(&mut simulation_controller, selector.index, *id);
                    
                    add_sender(&mut simulation_controller, *id, node.id);
                    add_sender(&mut simulation_controller, node.id, *id);
                    
                    node.connected_node_ids.push(*id);
                    
                    commands.spawn((
                        ScNode {
                            id: *id,
                            connected_node_ids: [node.id].to_vec(),
                            node_type: components::NodeType::Drone,
                            pdr: 0.0,
                        },
                        Sprite::from_image(asset_server.load("controller/Drone.png")),
                        Transform::from_xyz(0.0, 0.0, 1.0),
                        Pickable::default(),
                    )).with_children(|parent| {
                        parent.spawn((
                            DroneText,
                            Text2d::new(format!("id: {}  pdr: {}", *id, 0.0)),
                            Transform::from_xyz(0.0, -50.0, 1.0),
                        ));
                    });
                    
                    
                    nodes_writer.write(UpdateNodesEvent);
                    lines_writer.write(MakeLinesEvent);
                }
            }
        }
    }
}

pub fn change_spawn_target(
    trigger: Trigger<Pointer<Click>>,
    active_mode: Res<ActiveMode>,
    mut connect_selected: ResMut<SelectedNode>,
    mut nodes: Query<(Entity, &mut ScNode)>,
    state: Res<MainState>,
) {
    if let MainState::Sim = *state {
        if *active_mode != ActiveMode::Add {
            return;
        }
        
        let clicked_entity = trigger.target();
        
        let Ok((clicked_entity, _)) = nodes.get_mut(clicked_entity) else {
            return;
        };
        
        if connect_selected.0.is_none() {
            connect_selected.0 = Some(clicked_entity);
            return;
        }
        
        let entity = connect_selected.0.unwrap();
        
        if entity == clicked_entity {
            connect_selected.0 = None;
            return;
        }
    }
}



pub fn update_nodes(
    mut node_query: Query<(&ScNode, &mut Transform)>,
    mut reader: EventReader<UpdateNodesEvent>,
    mut writer: EventWriter<NodeMovedEvent>,
    state: Res<MainState>
) {
    if let MainState::Sim = *state {
        for _ in reader.read() {
            let mut rng = rand::thread_rng();
            
            let r = WINDOW_HEIGHT / 3.0;
            let drone_num = node_query.iter().filter(|(node, _)| matches!(node.node_type, components::NodeType::Drone)).count();
            if drone_num > 1 {
                let mut i = 0;
                for (node, mut transform) in node_query.iter_mut() {
                    if node.node_type == components::NodeType::Drone {
                        let theta = 2.0 * PI * i as f32 / drone_num as f32 + PI / 2.0;
                        let x = r * theta.cos();
                        let y = r * theta.sin();
                        
                        transform.translation.x = x + rng.gen_range(-30.0..30.0);
                        transform.translation.y = y + rng.gen_range(-30.0..30.0);
                        i += 1;
                        
                        writer.write(NodeMovedEvent {
                            node_id: node.id,
                            new_position: Some(transform.translation),
                        });
                    }
                }
            }
            
            let x = -(WINDOW_WIDTH / 8.0) * 3.0;
            let client_num = node_query.iter().filter(|(node, _)| matches!(node.node_type, components::NodeType::Client)).count();
            let dist = (WINDOW_HEIGHT - WINDOW_HEIGHT / 3.0) / (client_num as f32 - 1.0);
            
            let mut i = 0;
            for (node, mut transform) in node_query.iter_mut() {
                if node.node_type == components::NodeType::Client {
                    let y = WINDOW_HEIGHT / 6.0 - WINDOW_HEIGHT / 2.0 + (i as f32) * dist;
                    transform.translation.y = y;
                    transform.translation.x = x;
                    if client_num == 1 {
                        transform.translation.y = 0.0;
                        transform.translation.x = x;
                    }
                    i += 1;
                    
                    writer.write(NodeMovedEvent {
                        node_id: node.id,
                        new_position: Some(transform.translation),
                    });
                }
                
            }
            
            let x = (WINDOW_WIDTH / 8.0) * 3.0;
            let server_num = node_query.iter().filter(|(node, _)| matches!(node.node_type, components::NodeType::Server)).count() ;
            let dist = (WINDOW_HEIGHT - WINDOW_HEIGHT / 3.0) / (server_num as f32 - 1.0);
            let mut i = 0;
            for (node, mut transform) in node_query.iter_mut() {
                if node.node_type == components::NodeType::Server {
                    let y = WINDOW_HEIGHT / 6.0 - WINDOW_HEIGHT / 2.0 + (i as f32) * dist;
                    transform.translation.y = y;
                    transform.translation.x = x;
                    if server_num == 1 {
                        transform.translation.y = 0.0;
                        transform.translation.x = x;
                    }
                    i += 1;
                    
                    writer.write(NodeMovedEvent {
                        node_id: node.id,
                        new_position: Some(transform.translation),
                    });
                }
                
            }
            
        }
    }
}

pub fn make_lines(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    node_query: Query<(&ScNode, &mut Transform), With<ScNode>>,
    lines_query: Query<Entity, With<Mesh2d>>,
    mut reader: EventReader<MakeLinesEvent>,
    state: Res<MainState>,
){
    if let MainState::Sim = *state {
        for _ in reader.read() {
            for line in &mut lines_query.iter() {
                commands.entity(line).despawn();
            }
            
            for (node, transform) in node_query.iter() {
                for connected_node in node.connected_node_ids.iter() {
                    for (c_node, c_transform) in node_query.iter() {
                        if c_node.id == *connected_node {
                            create_line(
                                &mut commands,
                                &mut meshes,
                                &mut materials,
                                transform.translation.x, transform.translation.y,
                                c_transform.translation.x, c_transform.translation.y
                            );
                            //println!("creating line at {},{} - {},{}", transform.translation.x, transform.translation.y, c_transform.translation.x, c_transform.translation.y)
                        }
                    }
                }
            }
        }
    }
}

pub fn create_line(
    commands:  &mut Commands,
    meshes:  &mut ResMut<Assets<Mesh>>,
    materials:  &mut ResMut<Assets<ColorMaterial>>,
    x1: f32, y1: f32,
    x2: f32, y2: f32
) {
    let line_positions = vec![
        Vec3::new(x1, y1, -1.0),
        Vec3::new(x2, y2, -1.0),
    ];

    let mesh = Mesh::new(PrimitiveTopology::LineList,RenderAssetUsages::default())
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            line_positions
        );

    commands.spawn((
        Mesh2d(meshes.add(mesh)),
        MeshMaterial2d(materials.add(Color::from(BLACK))),
    ));
}




pub fn crash_target(
    trigger: Trigger<Pointer<Click>>,
    active_mode: Res<ActiveMode>,
    mut node_query: Query<(Entity, &mut ScNode)>,
    mut commands: Commands,
    mut nodes_writer: EventWriter<UpdateNodesEvent>,
    mut lines_writer: EventWriter<MakeLinesEvent>,
    mut moved_writer: EventWriter<NodeMovedEvent>,
    mut warn_query: Query<&mut Text, With<TextWarn>>,
    mut simulation_controller: ResMut<SimulationController>,
    state: Res<MainState>
) {
    if let MainState::Sim = *state {
    if *active_mode != ActiveMode::Crash {
        return;
    }

    let clicked_entity = trigger.target();

    let Ok((entity_to_remove, node_to_remove)) = node_query.get(clicked_entity) else {
        return;
    };

    if node_to_remove.node_type != components::NodeType::Drone {
        return;
    }

    let id_to_remove = node_to_remove.id;
    let connected_to_removed = node_to_remove.connected_node_ids.clone();

    let mut warn_text = warn_query.single_mut().unwrap();

    for &connected_id in &node_to_remove.connected_node_ids {
        if let Some((_, connected_node)) = node_query.iter().find(|(_, n)| n.id == connected_id) {
            match connected_node.node_type {
                components::NodeType::Client if connected_node.connected_node_ids.len() <= 1 => {
                    *warn_text = Text::from(format!("A Client must always be connected to at least 1 drone"));
                    return;
                }
                components::NodeType::Server if connected_node.connected_node_ids.len() <= 2 => {
                    *warn_text = Text::from(format!("A Server must always be connected to at least 2 drones"));
                    return;
                }
                components::NodeType::Drone => {

                    let mut topology = HashMap::<u8, Vec<u8>>::new();

                    for (_, node) in node_query.iter().filter(|(_, n)| n.id != id_to_remove) {

                        let filtered_ids = node
                            .connected_node_ids
                            .iter()
                            .filter(|&id| *id != node_to_remove.id)
                            .cloned()
                            .collect();

                        topology.insert(node.id, filtered_ids);
                    }

                    let mut visited = HashSet::new();
                    let mut stack = Vec::new();

                    for (_, node) in node_query.iter() {
                        if node.node_type == components::NodeType::Client || node.node_type == components::NodeType::Server {
                            stack.push(node.id);
                            visited.insert(node.id);
                        }
                    }

                    let drone_ids: Vec<u8> = node_query
                        .iter()
                        .filter(|(_, n)| n.node_type == components::NodeType::Drone && n.id != node_to_remove.id)
                        .map(|(_, n)| n.id)
                        .collect();

                    for &drone_id in &drone_ids {
                        let mut visited = HashSet::new();
                        let mut stack = vec![drone_id];
                        let mut connected_to_client_or_server = false;

                        while let Some(current) = stack.pop() {
                            if !visited.insert(current) {
                                continue;
                            }

                            if let Some((_, node)) = node_query.iter().find(|(_, n)| n.id == current) {
                                if node.node_type == components::NodeType::Client || node.node_type == components::NodeType::Server {
                                    connected_to_client_or_server = true;
                                    break;
                                }
                            }

                            if let Some(neighbors) = topology.get(&current) {
                                for &neighbor in neighbors {
                                    if !visited.contains(&neighbor) {
                                        stack.push(neighbor);
                                    }
                                }
                            }
                        }

                        if !connected_to_client_or_server {
                            *warn_text = Text::from(format!("No isolated Drones allowed"));
                            return;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    

    crash(&mut simulation_controller, id_to_remove, connected_to_removed);

    for (entity, mut node) in node_query.iter_mut() {
        if entity != entity_to_remove {
            node
                .connected_node_ids
                .retain(|&id| id != id_to_remove);
        }
    }

    moved_writer.write(NodeMovedEvent {
        node_id: id_to_remove,
        new_position: None,
    });
    commands.entity(entity_to_remove).despawn();


    nodes_writer.write(UpdateNodesEvent);
    lines_writer.write(MakeLinesEvent);

    }
}

pub fn manage_highlight(
    trigger: Trigger<Pointer<Over>>,
    active_mode: Res<ActiveMode>,
    mut sprite_query: Query<(&mut Sprite, &ScNode)>,
    state: Res<MainState>
) {
    if let MainState::Sim = *state {
    let Ok((mut sprite, node)) = sprite_query.get_mut(trigger.target()) else {
        return;
    };

    match *active_mode {
        ActiveMode::Crash => {
            if node.node_type == components::NodeType::Drone {
                sprite.color = Color::srgb(0.9, 0.0, 0.0);
            }
        }
        ActiveMode::Connect | ActiveMode::Add => {
            sprite.color = Color::srgb(0.0, 0.9, 0.0);
        }
        ActiveMode::Pdr => {
            if node.node_type == components::NodeType::Drone {
                sprite.color = Color::srgb(0.0, 0.0, 0.9);
            }
        }
        _ => {}
    }
    }
}

pub fn reset_highlight(
    trigger: Trigger<Pointer<Out>>,
    mut sprite_query: Query<(&mut Sprite, &ScNode)>,
    active_mode: Res<ActiveMode>,
    connect_selected: Res<SelectedNode>,
    state: Res<MainState>
) {
    if let MainState::Sim = *state {
    let exited_entity = trigger.target();

    let mut selected_id = None;
    if let Some(selected_entity) = connect_selected.0 {
        if let Ok((_, node)) = sprite_query.get(selected_entity) {
            selected_id = Some(node.id);
        }
    }

    if let Ok((mut sprite, exited_node)) = sprite_query.get_mut(exited_entity) {
        if Some(exited_node.id) == selected_id {
            if *active_mode == ActiveMode::Connect || *active_mode == ActiveMode::Add {
                sprite.color = Color::srgb(0.0, 0.6, 0.0);
            }else if *active_mode == ActiveMode::Pdr {
                sprite.color = Color::srgb(0.0, 0.0, 0.6);
            }

        } else {
            sprite.color = Color::WHITE;
        }
    }
    }   
}

pub fn change_pdr(
    mut reader: EventReader<ChangePdrEvent>,
    mut node_query: Query<&mut ScNode>,
    mut text_query: Query<(&ChildOf, &mut Text2d), With<DroneText>>,
    mut simulation_controller: ResMut<SimulationController>,
    state: Res<MainState>
) {
    if let MainState::Sim = *state{
    for ChangePdrEvent { entity, new_pdr } in reader.read() {
        if let Ok(mut node) = node_query.get_mut(*entity) {
            set_pdr(&mut simulation_controller, node.id, *new_pdr);
            node.pdr = *new_pdr;

            for (child_of, mut text) in &mut text_query {
                if child_of.parent() == *entity {
                    text.0 = format!("id: {}  pdr: {:.1}", node.id, node.pdr);
                }
            }
        }
    }
    }
}

pub fn change_pdr_target(
    trigger: Trigger<Pointer<Click>>,
    active_mode: Res<ActiveMode>,
    mut change_selected: ResMut<SelectedNode>,
    mut nodes: Query<(Entity, &mut ScNode)>,
    state: Res<MainState>,
) {
    if let MainState::Sim = *state {
    if *active_mode != ActiveMode::Pdr {
        return;
    }

    let clicked_entity = trigger.target();

    let Ok((clicked_entity, clicked_node)) = nodes.get_mut(clicked_entity) else {
        return;
    };

    if clicked_node.node_type != components::NodeType::Drone {
        return;
    }

    if change_selected.0.is_none() {
        change_selected.0 = Some(clicked_entity);
        return;
    }

    let entity = change_selected.0.unwrap();

    if entity == clicked_entity {
        change_selected.0 = None;
        return;
    }
    }
}

pub fn connect_nodes(
    trigger: Trigger<Pointer<Click>>,
    active_mode: Res<ActiveMode>,
    mut connect_selected: ResMut<SelectedNode>,
    mut nodes: Query<(Entity, &mut ScNode)>,
    mut sprite_query: Query<&mut Sprite>,
    mut warn_query: Query<&mut Text, With<TextWarn>>,
    mut writer: EventWriter<MakeLinesEvent>,
    mut simulation_controller: ResMut<SimulationController>,
    state: Res<MainState>,
) {
    if let MainState::Sim = *state {
    if *active_mode != ActiveMode::Connect {
        return;
    }

    let mut warn_text = warn_query.single_mut().unwrap();

    let clicked_entity = trigger.target();

    let Ok((clicked_entity, _)) = nodes.get_mut(clicked_entity) else {
        return;
    };

    if connect_selected.0.is_none() {
        connect_selected.0 = Some(clicked_entity);
        return;
    }

    let first_entity = connect_selected.0.unwrap();

    if first_entity == clicked_entity {
        connect_selected.0 = None;
        return;
    }

    let mut first_node_opt = None;
    let mut second_node_opt = None;

    for (entity, node) in &mut nodes {
        if entity == first_entity {
            first_node_opt = Some(node);
        } else if entity == clicked_entity {
            second_node_opt = Some(node);
        }

        if first_node_opt.is_some() && second_node_opt.is_some() {
            break;
        }
    }

    if let (Some(mut first_node), Some(mut second_node)) = (first_node_opt, second_node_opt) {
        if !first_node.connected_node_ids.contains(&second_node.id) && !second_node.connected_node_ids.contains(&first_node.id){
            if first_node.node_type == components::NodeType::Drone || second_node.node_type == components::NodeType::Drone {

                if (first_node.node_type == components::NodeType::Client && first_node.connected_node_ids.len() >= 2) ||
                    (second_node.node_type == components::NodeType::Client && second_node.connected_node_ids.len() >= 2){
                    *warn_text = Text::from("Client can have at most 2 drones connected");
                    return;
                }

                add_sender(&mut simulation_controller, first_node.id, second_node.id);
                add_sender(&mut simulation_controller, second_node.id, first_node.id);

                first_node.connected_node_ids.push(second_node.id);
                second_node.connected_node_ids.push(first_node.id);
            }else{
                *warn_text = Text::from("Invalid connection");
            }
        }else{
            *warn_text = Text::from("Nodes already connected");
        }
    }
    writer.write(MakeLinesEvent);
    if let Ok(mut sprite) = sprite_query.get_mut(first_entity) {
        sprite.color = Color::WHITE;
    }

    connect_selected.0 = None;
    }
}

pub fn button_system(
    mut query: Query<(&Interaction, &mut BackgroundColor, &ButtonColors, Option<&ButtonLabel>)>,
    active_mode: Res<ActiveMode>,
    state: Res<MainState>,
) {
    if let MainState::Sim = *state {
    for (interaction, mut color, colors, label_opt) in &mut query {
        let is_active = match (active_mode.as_ref(), label_opt) {
            (ActiveMode::Add, Some(ButtonLabel::Add)) => true,
            (ActiveMode::Crash, Some(ButtonLabel::Crash)) => true,
            (ActiveMode::Connect, Some(ButtonLabel::Connect)) => true,
            (ActiveMode::Pdr, Some(ButtonLabel::Pdr)) => true,
            _ => false,
        };

        if is_active {
            *color = BackgroundColor(Color::srgb(0.3, 0.3, 0.3));
        } else {
            let c = match *interaction {
                Interaction::Pressed => colors.pressed,
                Interaction::Hovered => colors.hovered,
                Interaction::None => colors.normal,
            };
            *color = BackgroundColor(c);
        }
    }
    }
}

pub fn button_action(
    mut drone_add_query: Query<&mut Visibility, (With<DroneAdd>, Without<ConfirmShown>, Without<TextBox>)>,
    mut textbox_query: Query<&mut Visibility, (With<TextBox>, Without<ConfirmShown>, Without<DroneAdd>)>,
    mut confirm_query: Query<&mut Visibility, (With<ConfirmShown>, Without<DroneAdd>, Without<TextBox>)>,
    mut textbox_top_query: Query<&mut Text, (With<TextboxTopText>, Without<TextWarn>)>,
    mut warn_query: Query<&mut Text, (With<TextWarn>, Without<TextboxTopText>)>,
    interaction_query: Query<(&Interaction, Option<&ButtonLabel>), (Changed<Interaction>, With<Button>)>,
    mut text_query: Query<(&mut TextInputValue, &mut TextInputInactive), With<DroneIdInput>>,
    mut node_query: Query<(&ScNode, &mut Sprite)>,
    mut spawn_writer: EventWriter<SpawnDroneEvent>,
    mut pdr_writer: EventWriter<ChangePdrEvent>,
    mut selected_node: ResMut<SelectedNode>,
    mut active_mode: ResMut<ActiveMode>,
    state: Res<MainState>,
) {
    if let MainState::Sim = *state {
    for (interaction, label) in &interaction_query {
        if *interaction != Interaction::Pressed {
            continue;
        }

        let mut textbox_visibility = textbox_query.single_mut().unwrap();
        let mut drone_add_visibility = drone_add_query.single_mut().unwrap();
        let mut confirm_visibility = confirm_query.single_mut().unwrap();
        let mut textbox_top_text = textbox_top_query.single_mut().unwrap();
        let mut warn_text = warn_query.single_mut().unwrap();

        match label {
            Some(ButtonLabel::Add) => {
                *active_mode = ActiveMode::Add;
                *drone_add_visibility = Visibility::Visible;
                *textbox_visibility = Visibility::Visible;
                *confirm_visibility = Visibility::Visible;
                *textbox_top_text = Text::from("Drone ID");

                if let Some(entity) = selected_node.0 {
                    if let Ok((_, mut sprite)) = node_query.get_mut(entity) {
                        sprite.color = Color::WHITE;
                    }
                    selected_node.0 = None;
                }

                if let Ok((mut value, mut inactive)) = text_query.single_mut() {
                    value.0.clear();
                    inactive.0 = false;
                }
            }
            Some(ButtonLabel::Crash) => {
                *active_mode = ActiveMode::Crash;
                *drone_add_visibility = Visibility::Hidden;
                *textbox_visibility = Visibility::Hidden;
                *confirm_visibility = Visibility::Visible;

                if let Some(entity) = selected_node.0 {
                    if let Ok((_, mut sprite)) = node_query.get_mut(entity) {
                        sprite.color = Color::WHITE;
                    }
                    selected_node.0 = None;
                }

                if let Ok((mut value, mut inactive)) = text_query.single_mut() {
                    value.0.clear();
                    inactive.0 = true;
                }
            }
            Some(ButtonLabel::Connect) => {
                *active_mode = ActiveMode::Connect;
                *drone_add_visibility = Visibility::Hidden;
                *textbox_visibility = Visibility::Hidden;
                *confirm_visibility = Visibility::Visible;

                if let Some(entity) = selected_node.0 {
                    if let Ok((_, mut sprite)) = node_query.get_mut(entity) {
                        sprite.color = Color::WHITE;
                    }
                    selected_node.0 = None;
                }

                if let Ok((mut value, mut inactive)) = text_query.single_mut() {
                    value.0.clear();
                    inactive.0 = true;
                }
            }
            Some(ButtonLabel::Pdr) => {
                *active_mode = ActiveMode::Pdr;
                *drone_add_visibility = Visibility::Hidden;
                *textbox_visibility = Visibility::Visible;
                *confirm_visibility = Visibility::Visible;
                *textbox_top_text = Text::from("Set PDR");

                if let Some(entity) = selected_node.0 {
                    if let Ok((_, mut sprite)) = node_query.get_mut(entity) {
                        sprite.color = Color::WHITE;
                    }
                    selected_node.0 = None;
                }

                if let Ok((mut value, mut inactive)) = text_query.single_mut() {
                    value.0.clear();
                    inactive.0 = false;
                }
            }
            Some(ButtonLabel::Done) => {
                let mut warn = false;
                //if matches!(*drone_add_visibility, Visibility::Visible) {
                if let Ok((mut value, mut inactive)) = text_query.single_mut() {
                    if *active_mode == ActiveMode::Add {
                        if let Some(entity) = selected_node.0 {
                            if let Ok(id) = value.0.parse::<u8>() {
                                if !node_query.iter().any(|(node, _)| node.id == id) {
                                    //info!("Drone ID: {}", value.0);
                                    spawn_writer.write(SpawnDroneEvent { id, connect_to: entity });
                                } else {
                                    warn = true;
                                    *warn_text = Text::from(format!("Duplicate ID: '{}'", value.0));
                                    //warn!("Duplicate ID: '{}'", value.0);
                                }
                            } else {
                                warn = true;
                                *warn_text = Text::from(format!("Invalid ID: '{}'", value.0));
                                //warn!("Invalid ID: '{}'", value.0);
                            }
                        }else{
                            warn = true;
                            *warn_text = Text::from("Drone must be connected to at least 1 other node");
                        }
                    } else if *active_mode == ActiveMode::Pdr {
                        if let Some(entity) = selected_node.0 {
                            if let Ok((node, _)) = node_query.get(entity) {
                                if node.node_type == components::NodeType::Drone {
                                    if let Ok(pdr_val) = value.0.parse::<f32>()  {
                                        if pdr_val >= 0.0 && pdr_val <= 1.0 {
                                            pdr_writer.write(ChangePdrEvent {
                                                entity,
                                                new_pdr: pdr_val,
                                            });
                                        }else{
                                            warn = true;
                                            *warn_text = Text::from(format!("Invalid PDR value: '{}' (must be between 0.0 and 1.0)", value.0));
                                            //warn!("Invalid PDR value: '{}' (must be between 0.0 and 1.0)", value.0);
                                        }
                                    } else {
                                        warn = true;
                                        *warn_text = Text::from(format!("Invalid PDR value: '{}'", value.0));
                                        //warn!("Invalid PDR value: '{}'", value.0);
                                    }
                                    //println!("{}", value.0);
                                } else {
                                    warn!("Not a Drone");
                                }
                            } else {
                                warn!("Entity Deleted");
                            }
                        } else {
                            warn = true;
                            *warn_text = Text::from("No Drone Selected");
                            //warn!("No Drone Selected");
                        }
                    }

                    if !warn{
                        *warn_text = Text::from("");
                    }

                    value.0.clear();
                    inactive.0 = true;
                }
                //}

                *active_mode = ActiveMode::None;

                if let Some(entity) = selected_node.0 {
                    if let Ok((_, mut sprite)) = node_query.get_mut(entity) {
                        sprite.color = Color::WHITE;
                    }
                    selected_node.0 = None;
                }

                *textbox_visibility = Visibility::Hidden;
                *drone_add_visibility = Visibility::Hidden;
                *confirm_visibility = Visibility::Hidden;
            }
            _ => {}
        }
    }
    }
}

pub fn update_selector(
    mut interaction_query: Query<(&Interaction, &SelectorDirection), (Changed<Interaction>, With<Button>)>,
    mut selector_query: Query<(&mut Text, &mut DroneSelector)>,
    state: Res<MainState>,
) {
    if let MainState::Sim = *state {
    for (interaction, direction) in &mut interaction_query {
        if *interaction != Interaction::Pressed {
            continue;
        }

        for (mut display_text, mut selector) in &mut selector_query {
            match direction {
                SelectorDirection::Left if selector.index > 0 => {
                    selector.index -= 1;
                }
                SelectorDirection::Right if selector.index + 1 < DRONE_NAMES.len() => {
                    selector.index += 1;
                }
                _ => {}
            }

            *display_text = Text::new(DRONE_NAMES[selector.index]);
        }
    }
    }
}

pub fn crossbeam_listener(
    mut create_writer: EventWriter<PacketCreateEvent>,
    mut add_writer: EventWriter<PacketAddHopEvent>,
    mut simulation_controller: ResMut<SimulationController>,
    state: Res<MainState>
){
    if let MainState::Sim = *state {
    while let Ok(event) = simulation_controller.receiver_client_server_event.try_recv(){
        //println!("client or server sent a packet");
        create_writer.write(PacketCreateEvent::NodeEvent(event));

    }
    while let Ok(event) = simulation_controller.receiver_drone_event.try_recv() {
        //println!("sent addhop event: {:?}",event);
        if let DroneEvent::ControllerShortcut(ref packet) = event {
            shortcut(&mut simulation_controller, packet.clone());
        }
        create_writer.write(PacketCreateEvent::DroneEvent(event.clone()));
        add_writer.write(PacketAddHopEvent{ drone_event: event});
    }
    }
}

pub fn packet_spawn(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    //mut packet_query: Query<(&mut Sprite, &mut HopQueue, &mut PacketInfo)>,
    node_query: Query<(Entity, &ScNode, &mut Transform)>,
    mut reader: EventReader<PacketCreateEvent>,
    state: Res<MainState>,
){
    if let MainState::Sim = *state {
    for create_event in reader.read() {
        //let mut spawned: Vec<(u64,Vec<NodeId>)> = Vec::new();
        //let NodeEvent::PacketSent(packet) = &create_event.node_event;
        match create_event {
            PacketCreateEvent::NodeEvent(NodeEvent::PacketSent(packet)) => {
                let start_id = packet.routing_header.hops[packet.routing_header.hop_index - 1];
                let end_id = packet.routing_header.hops[packet.routing_header.hop_index];

                if let Some((_, _, start_transform)) = node_query.iter().find(|(_, node, _)| node.id == start_id) {
                    if let Some((_, _, end_transform)) = node_query.iter().find(|(_, node, _)| node.id == end_id) {
                        commands.spawn((
                            Sprite::from_image(asset_server.load("controller/packet.png")),
                            Transform::from_translation(start_transform.translation).with_scale(Vec3::splat(0.5)),
                            PacketMotion {
                                start: start_transform.translation,
                                end: end_transform.translation,
                                progress: 0.0,
                            },
                            PacketInfo {
                                session_id: packet.session_id.clone(),
                                hops: packet.routing_header.hops.clone(),
                                last_hop_index: 1,
                            },
                            HopQueue(VecDeque::from([(start_id, Some(end_id))]))
                        ));
                    }
                }
            }
            /*

            PacketCreateEvent::DroneEvent(create_event) => {
                match create_event {
                    DroneEvent::PacketSent(packet) => {

                        match packet.pack_type
                        {
                            PacketType::Nack(_) => {
                                let already_exists_this_frame = spawned
                                    .iter()
                                    .any(|(s, h)| *s == packet.session_id && *h == packet.routing_header.hops );

                                let already_exists = packet_query.iter().any(|(_, hop_queue, info)| {
                                    info.session_id == packet.session_id &&
                                        info.hops == packet.routing_header.hops

                                });
                                if !already_exists && !already_exists_this_frame {
                                    spawned.push((packet.session_id,packet.routing_header.hops.clone()));
                                    let start_id = packet.routing_header.hops[packet.routing_header.hop_index - 1];
                                    let end_id = packet.routing_header.hops[packet.routing_header.hop_index];

                                    if let Some((_, _, start_transform)) = node_query.iter().find(|(_, node, _)| node.id == start_id) {
                                        if let Some((_, _, end_transform)) = node_query.iter().find(|(_, node, _)| node.id == end_id) {
                                            commands.spawn((
                                                Sprite::from_image(asset_server.load("controller/packet.png")),
                                                Transform::from_translation(start_transform.translation),
                                                PacketMotion {
                                                    start: start_transform.translation,
                                                    end: end_transform.translation,
                                                    progress: 0.0,
                                                },
                                                PacketInfo {
                                                    session_id: packet.session_id.clone(),
                                                    hops: packet.routing_header.hops.clone(),
                                                    last_hop_index: 1,
                                                },
                                                HopQueue(VecDeque::from([(start_id, Some(end_id))]))
                                            ));
                                        }
                                    }
                                }
                            }
                            PacketType::FloodResponse(_) => {
                                let already_exists_this_frame = spawned
                                    .iter()
                                    .any(|(s, h)| *s == packet.session_id && *h == packet.routing_header.hops);

                                let already_exists = packet_query.iter().any(|(_, hop_queue, info)| {
                                    info.session_id == packet.session_id &&
                                        info.hops == packet.routing_header.hops
                                });


                                if !already_exists && !already_exists_this_frame {
                                    spawned.push((packet.session_id,packet.routing_header.hops.clone()));

                                    let start_id = packet.routing_header.hops[packet.routing_header.hop_index - 1];
                                    let end_id = packet.routing_header.hops[packet.routing_header.hop_index];

                                    if let Some((_, _, start_transform)) = node_query.iter().find(|(_, node, _)| node.id == start_id) {
                                        if let Some((_, _, end_transform)) = node_query.iter().find(|(_, node, _)| node.id == end_id) {
                                            commands.spawn((
                                                Sprite::from_image(asset_server.load("controller/packet.png")),
                                                Transform::from_translation(start_transform.translation),
                                                PacketMotion {
                                                    start: start_transform.translation,
                                                    end: end_transform.translation,
                                                    progress: 0.0,
                                                },
                                                PacketInfo {
                                                    session_id: packet.session_id.clone(),
                                                    hops: packet.routing_header.hops.clone(),
                                                    last_hop_index: 1,
                                                },
                                                HopQueue(VecDeque::from([(start_id, Some(end_id))]))
                                            ));
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }*/
            _ => {}
        }
    }
    }
}

pub fn packet_add_hop(
    mut reader: EventReader<PacketAddHopEvent>,
    mut query: Query<(&mut Sprite, &mut HopQueue, &mut PacketInfo)>,
    state: Res<MainState>,
) {
    if let MainState::Sim = *state {
    for add_hop_event in reader.read() {
        match &add_hop_event.drone_event {
            DroneEvent::PacketSent(packet) => {
                for (_, mut hop_queue, mut info) in &mut query {
                    if info.session_id == packet.session_id &&
                        info.hops == packet.routing_header.hops &&
                        info.last_hop_index + 1 == packet.routing_header.hop_index
                    {
                        let start_id = packet.routing_header.hops[packet.routing_header.hop_index - 1];
                        let end_id = packet.routing_header.hops[packet.routing_header.hop_index];
                        hop_queue.0.push_back((start_id, Some(end_id)));
                        info.last_hop_index += 1;
                        break;
                    }
                }
            }
            DroneEvent::PacketDropped(packet) => {
                //println!("packet_dropped {:?}",packet);
                for (_, mut hop_queue, info) in &mut query {
                    if info.session_id == packet.session_id &&
                        info.hops == packet.routing_header.hops &&
                        info.last_hop_index == packet.routing_header.hop_index
                    {
                        let start_id = packet.routing_header.hops[packet.routing_header.hop_index - 1];
                        hop_queue.0.push_back((start_id, None));
                        //info.last_hop_index += 1;

                        break;
                    }
                }
            }
            DroneEvent::ControllerShortcut(packet) => {
                for (mut sprite, mut hop_queue, info) in &mut query {
                    if info.session_id == packet.session_id &&
                        info.hops == packet.routing_header.hops &&
                        info.last_hop_index == packet.routing_header.hop_index
                    {
                        let start_id = packet.routing_header.hops[packet.routing_header.hop_index - 1];
                        let end_id = packet.routing_header.hops[packet.routing_header.hops.len() - 1];
                        hop_queue.0.push_back((start_id, Some(end_id)));
                        //info.last_hop_index += 1;
                        sprite.color = Color::srgb(0.8, 0.8, 1.0);

                        break;
                    }
                }
            }
        }
    }
    }
}

pub fn packet_move(
    time: Res<Time>,
    mut commands: Commands,
    node_query: Query<(&ScNode, &Transform), Without<PacketMotion>>,
    mut packet_query: Query<(Entity, &mut Transform, &mut Sprite, &mut PacketMotion, &mut HopQueue), Without<ScNode>>,
    state: Res<MainState>,
) {
    if let MainState::Sim = *state {
    for (entity, mut transform, mut sprite, mut motion, mut queue) in &mut packet_query {
        motion.progress += PACKET_SPEED * time.delta_secs() / motion.start.distance(motion.end);
        motion.progress = motion.progress.clamp(0.0, 1.0);

        if sprite.color.alpha() >= 1.0 {
            transform.translation = motion.start.lerp(motion.end, motion.progress);
        }

        if motion.progress >= 1.0 {
            if let Some((_, next_end_id)) = queue.0.front() {
                match next_end_id {
                    Some(id) => {
                        if let Some((_, end_transform)) = node_query.iter().find(|(node, _)| node.id == *id) {

                            motion.start = transform.translation;
                            motion.end = end_transform.translation;
                            motion.progress = 0.0;
                        }
                    }
                    None => {
                        let alpha = (sprite.color.alpha() - time.delta_secs() * 0.5).clamp(0.0, 1.0);
                        sprite.color = Color::srgba(1.0, 1.0, 1.0, alpha);

                        transform.translation.y -= 5.0;

                        if alpha <= 0.0 {
                            commands.entity(entity).despawn();
                        }
                    }
                }
            }
            if sprite.color.alpha() >= 1.0 {
                if queue.0.is_empty() {
                    commands.entity(entity).despawn();
                }
                queue.0.pop_front();
            }
        }
    }
    }
}

pub fn update_packet_ends(
    mut reader: EventReader<NodeMovedEvent>,
    mut packet_query: Query<(&mut PacketMotion, &HopQueue)>,
    state: Res<MainState>,
) {
    if let MainState::Sim = *state {
    for event in reader.read() {
        for (mut motion, queue) in &mut packet_query {
            if let Some(&(start_id, end_id_opt)) = queue.0.front() {
                if event.node_id == start_id {
                    if let Some(new_pos) = event.new_position {
                        motion.start = new_pos;
                    }
                }

                if let Some(end_id) = end_id_opt {
                    if event.node_id == end_id {
                        if let Some(new_pos) = event.new_position {
                            motion.end = new_pos;
                        }
                    }
                }
            }
        }
    }
    }
}
/*
pub fn update_packet_ends(
    mut reader: EventReader<NodeMovedEvent>,
    mut query: Query<&mut PacketMotion>,
) {
    for event in reader.read() {
        for mut motion in &mut query {
            if motion.start_id == event.node_id {
                match event.new_position {
                    Some(pos) => motion.start = pos,
                    None => motion.falling = true,
                }
            }

            if motion.end_id == event.node_id {
                match event.new_position {
                    Some(pos) => motion.end = pos,
                    None => motion.falling = true,
                }
            }
        }
    }
}


 */
