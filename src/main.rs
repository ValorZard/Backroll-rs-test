use macroquad::prelude::*;
use tracing_subscriber::*;
use backroll::*;
use backroll_transport_udp::*;
use bevy_tasks::*;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use bytemuck::{Pod, Zeroable};
//use clap::{Arg, App};

#[macro_use]
extern crate bitflags;

#[derive(Clone)]
pub struct Player {
    position: Vec2,
    velocity: Vec2,
    size: Vec2,
    handle: BackrollPlayerHandle, // the network id 
}

bitflags! {
    #[derive(Default, Pod, Zeroable)]
    #[repr(C)]
    pub struct Input: u32 {
        // bit shift the stuff in the input struct
        const UP = 1<<0;
        const DOWN = 1<<1;
        const LEFT = 1<<2;
        const RIGHT = 1<<3;
    }
}


pub fn player_physics_update(player : &mut Player, input : Input)
{
    if input.contains(Input::RIGHT) {
        player.velocity.x = 10.0;
    } else if input.contains(Input::LEFT) {
        player.velocity.x = -10.0;
    } else {
        player.velocity.x = 0.0;
    }

    if input.contains(Input::UP) {
        player.velocity.y = -10.0;
    } else if input.contains(Input::DOWN) {
        player.velocity.y = 10.0;
    } else {
        player.velocity.y = 0.0;
    }

    let velocity = player.velocity;

    player.position += velocity;
}

// in this game, since its incredibly simple, the "gamestate" is just a vector of players

struct TestBackrollConfig;

impl BackrollConfig for TestBackrollConfig{
    type Input = Input;
    type State = Vec<Player>;
    const MAX_PLAYERS_PER_MATCH: usize = 2;
    const RECOMMENDATION_INTERVAL: u32 = 420;
}

struct TestSessionCallbacks{
    players : Vec<Player>,
}

impl SessionCallbacks<TestBackrollConfig> for TestSessionCallbacks{
    fn save_state(&mut self) -> (Vec<Player>, Option<u64>)
    {
        (self.players.clone(), None)
    }

    fn load_state(&mut self, players : &Vec<Player>)
    {
        self.players = players.clone();
    }
    
    fn advance_frame(&mut self, input : GameInput<Input>)
    {
        for player in self.players.iter_mut()
        {
            // physics update
            // we have to do a match function to make sure input is "safe"
            player_physics_update(player, match input.get(player.handle) {
                Ok(input) => *input,
                Err(err) => panic!("Input somehow failed"),
            });
        }
    }

    fn handle_event(&mut self, event : BackrollEvent)
    {
        println!("This isnt implemented yet lmao.");
    }
}


pub fn render_player(player : & Player)
{
    draw_rectangle(
        player.position.x,
        player.position.y,
        player.size.x,
        player.size.y,
        GREEN,
    );
}


#[macroquad::main("BasicShapes")]
async fn main() {
    // do logging
    tracing_subscriber::fmt()
        .with_level(true)
        .with_thread_ids(true)
        .with_timer(tracing_subscriber::fmt::time::ChronoUtc::rfc3339())
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .compact()
        .init();

    // set up networking
    
    let task_pool = TaskPool::new();

    // will panic if fails to bind :)
    let connection_manager = UdpManager::bind(task_pool.clone(), "127.0.0.1:420").unwrap();

    // leads to memory exhaustion if someone sends a shitton of packets
    let connect_config = UdpConnectionConfig::unbounded("127.0.0.1:421".parse().unwrap());

    // actual abstraction layer part
    let remote_peer = connection_manager.connect(connect_config);

    // backroll config

    let mut session_builder = P2PSession::<TestBackrollConfig>::build();
    let local_handle = session_builder.add_player(BackrollPlayer::Local);
    let online_handle = session_builder.add_player(BackrollPlayer::Remote(remote_peer));

    // set up the actual game

    let mut local_player = Player {
        position: Vec2::new(screen_width() / 2.0 - 60.0, 100.0),
        velocity: Vec2::new(0.0, 0.0),
        size: Vec2::new(50.0, 50.0),
        handle: local_handle,
    };

    let mut online_player = Player {
        position: Vec2::new(screen_width() / 2.0 - 60.0, 100.0),
        velocity: Vec2::new(0.0, 0.0),
        size: Vec2::new(50.0, 50.0),
        handle: online_handle
    };

    // setting the "gamestate" in the callbacks
    let mut callbacks = TestSessionCallbacks{
        players: vec![local_player, online_player],
    };

    // start the actual session (can fail)
    let mut session = match session_builder.start(task_pool) {
        Ok(session) => session,
        Err(err) => panic!("Session failed"),
    };

    loop {
        clear_background(BLUE);

        let mut local_input = Input::empty();

        // local input handling
        {
            if is_key_down(KeyCode::Right) {
                local_input.insert(Input::RIGHT);
            } else if is_key_down(KeyCode::Left) {
                local_input.insert(Input::LEFT);
            }

            if is_key_down(KeyCode::Up) {
                local_input.insert(Input::UP);
            } else if is_key_down(KeyCode::Down) {
                local_input.insert(Input::DOWN);
            }
        }

        session.add_local_input(callbacks.players[0].handle, local_input);

        session.advance_frame(&mut callbacks);

        // render players
        {
            // player local
            render_player(&callbacks.players[0]);
            render_player(&callbacks.players[1]);
        }

        draw_text("IT WORKS!", 20.0, 20.0, 30.0, RED);

        next_frame().await
    }
}
