use rand::Rng;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

// Direction the snake can move
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

// Game state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    Playing,
    GameOver,
    Won,
}

// Type for storing positions on the game board
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

impl Position {
    pub fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }
}

// Main Snake game struct
pub struct Snake {
    // The snake's body as a sequence of positions (head is at the front)
    body: VecDeque<Position>,
    // Direction the snake is moving
    direction: Direction,
    // Position of the food
    food: Position,
    // Current game state
    state: GameState,
    // Width and height of the game area
    width: usize,
    height: usize,
    // Score
    score: usize,
    // Max score to win
    max_score: usize,
    // Last time the snake moved (for controlling speed)
    last_update: Instant,
    // Movement speed
    speed: Duration,
}

impl Snake {
    // Create a new snake game with the given dimensions
    pub fn new(width: usize, height: usize) -> Self {
        let mut rng = rand::thread_rng();
        let start_x = width / 4;
        let start_y = height / 2;
        
        // Initialize snake with 3 segments
        let mut body = VecDeque::new();
        body.push_back(Position::new(start_x, start_y));
        body.push_back(Position::new(start_x - 1, start_y));
        body.push_back(Position::new(start_x - 2, start_y));
        
        // Generate initial food position (not on snake)
        let mut food_x;
        let mut food_y;
        loop {
            food_x = rng.gen_range(1..width - 1);
            food_y = rng.gen_range(1..height - 1);
            let food_pos = Position::new(food_x, food_y);
            if !body.contains(&food_pos) {
                break;
            }
        }
        
        Self {
            body,
            direction: Direction::Right,
            food: Position::new(food_x, food_y),
            state: GameState::Playing,
            width,
            height,
            score: 0,
            max_score: (width * height) / 4, // Win when snake fills 1/4 of the board
            last_update: Instant::now(),
            speed: Duration::from_millis(100), // Initial speed (faster)
        }
    }
    
    // Change the snake's direction (if valid)
    pub fn change_direction(&mut self, direction: Direction) {
        // Prevent 180-degree turns (snake can't go back on itself)
        let invalid_turn = match (self.direction, direction) {
            (Direction::Up, Direction::Down) => true,
            (Direction::Down, Direction::Up) => true,
            (Direction::Left, Direction::Right) => true,
            (Direction::Right, Direction::Left) => true,
            _ => false,
        };
        
        if !invalid_turn {
            self.direction = direction;
        }
    }
    
    // Update the game state
    pub fn update(&mut self) -> bool {
        if self.state != GameState::Playing {
            return false;
        }
        
        // Only update at specific intervals for speed control
        let now = Instant::now();
        if now.duration_since(self.last_update) < self.speed {
            return false;
        }
        self.last_update = now;
        
        // Get current head position
        let head = self.body.front().unwrap().clone();
        
        // Calculate new head position based on direction
        let new_head = match self.direction {
            Direction::Up => Position::new(head.x, head.y.saturating_sub(1)),
            Direction::Down => Position::new(head.x, head.y + 1),
            Direction::Left => Position::new(head.x.saturating_sub(1), head.y),
            Direction::Right => Position::new(head.x + 1, head.y),
        };
        
        // Check for collisions with walls
        if new_head.x >= self.width || new_head.y >= self.height {
            self.state = GameState::GameOver;
            return true;
        }
        
        // Check for collision with self (excluding tail which will move)
        for i in 0..self.body.len() - 1 {
            if new_head == self.body[i] {
                self.state = GameState::GameOver;
                return true;
            }
        }
        
        // Add new head to the snake
        self.body.push_front(new_head);
        
        // Check if snake eats food
        if new_head == self.food {
            // Increase score
            self.score += 1;
            
            // Check for win condition
            if self.score >= self.max_score {
                self.state = GameState::Won;
                return true;
            }
            
            // Increase speed more aggressively
            self.speed = Duration::from_millis((self.speed.as_millis() as f64 * 0.90) as u64);
            
            // Generate new food position
            let mut rng = rand::thread_rng();
            loop {
                let food_x = rng.gen_range(1..self.width - 1);
                let food_y = rng.gen_range(1..self.height - 1);
                let food_pos = Position::new(food_x, food_y);
                
                if !self.body.contains(&food_pos) {
                    self.food = food_pos;
                    break;
                }
            }
        } else {
            // If not eating, remove the tail
            self.body.pop_back();
        }
        
        true // Game updated
    }
    
    // Get the snake's body positions (for rendering)
    pub fn body(&self) -> &VecDeque<Position> {
        &self.body
    }
    
    // Get the food position
    pub fn food(&self) -> Position {
        self.food
    }
    
    // Get the current score
    pub fn score(&self) -> usize {
        self.score
    }
    
    // Get the current game state
    pub fn state(&self) -> GameState {
        self.state
    }
    
    // Reset the game
    pub fn reset(&mut self) {
        *self = Snake::new(self.width, self.height);
    }
}