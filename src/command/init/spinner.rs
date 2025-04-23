use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use termimad::crossterm::{
    cursor::{position, Hide, MoveTo, Show},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType},
};

/// A simple command line spinner component
pub struct Spinner {
    #[allow(dead_code)]
    message: Arc<Mutex<String>>,
    active: Arc<Mutex<bool>>,
    handle: Option<thread::JoinHandle<()>>,
    initial_position: Arc<Mutex<(u16, u16)>>,
}

impl Spinner {
    /// Create a new spinner with the given message and animation frames
    pub fn new(message: &str, frames: Vec<char>) -> Self {
        let message = Arc::new(Mutex::new(message.to_string()));
        let active = Arc::new(Mutex::new(true));
        let frames_clone = frames.clone();
        let message_clone = Arc::clone(&message);
        let active_clone = Arc::clone(&active);

        // Get initial cursor position
        let mut stdout = io::stdout();
        let initial_position = if let Ok((_, row)) = position() {
            execute!(stdout, MoveTo(0, row + 1)).unwrap();
            (0, row + 1)
        } else {
            (0, 0)
        };

        let initial_position = Arc::new(Mutex::new(initial_position));
        let initial_position_clone = Arc::clone(&initial_position);

        // Hide the cursor while the spinner is active
        let _ = execute!(io::stdout(), Hide);

        let handle = thread::spawn(move || {
            let mut stdout = io::stdout();
            let mut frame_index = 0;
            let pos = *initial_position_clone.lock().unwrap();

            while *active_clone.lock().unwrap() {
                let frame = frames_clone[frame_index];
                frame_index = (frame_index + 1) % frames_clone.len();

                let current_message = message_clone.lock().unwrap().clone();

                // Always return to initial position, clear the line, and print the updated spinner
                execute!(
                    stdout,
                    MoveTo(pos.0, pos.1),
                    Clear(ClearType::CurrentLine),
                    SetForegroundColor(Color::Cyan),
                    Print(format!("{} {}", frame, current_message)),
                    ResetColor
                )
                .unwrap();
                stdout.flush().unwrap();

                thread::sleep(Duration::from_millis(80));
            }
        });

        Self {
            message,
            active,
            handle: Some(handle),
            initial_position,
        }
    }

    /// Update the spinner message
    #[allow(dead_code)]
    pub fn update(&self, message: String) {
        let mut current_message = self.message.lock().unwrap();
        *current_message = message;
    }

    /// Stop the spinner with a success message (green)
    pub fn success(&self, message: &str) {
        // Only proceed if the spinner is still active
        let mut active = self.active.lock().unwrap();
        if !*active {
            // Already stopped
            return;
        }

        // Mark as inactive
        *active = false;

        if let Some(handle) = &self.handle {
            handle.thread().unpark();
        }

        let mut stdout = io::stdout();
        let pos = *self.initial_position.lock().unwrap();

        execute!(
            stdout,
            MoveTo(pos.0, pos.1),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::Green),
            Print(format!("✓ {}", message)),
            ResetColor,
            Print("\n")
        )
        .unwrap();
        stdout.flush().unwrap();

        // Show the cursor again
        let _ = execute!(io::stdout(), Show);
    }

    /// Stop the spinner with an error message (red)
    #[allow(dead_code)]
    pub fn error(&self, message: &str) {
        // Only proceed if the spinner is still active
        let mut active = self.active.lock().unwrap();
        if !*active {
            // Already stopped
            return;
        }

        // Mark as inactive
        *active = false;

        if let Some(handle) = &self.handle {
            handle.thread().unpark();
        }

        let mut stdout = io::stdout();
        let pos = *self.initial_position.lock().unwrap();

        execute!(
            stdout,
            MoveTo(pos.0, pos.1),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::Red),
            Print(format!("✗ {}", message)),
            ResetColor,
            Print("\n")
        )
        .unwrap();
        stdout.flush().unwrap();

        // Show the cursor again
        let _ = execute!(io::stdout(), Show);
    }

    /// Stop the spinner without any message
    pub fn stop(&self) {
        // Only proceed if the spinner is still active
        let mut active = self.active.lock().unwrap();
        if !*active {
            // Already stopped
            return;
        }

        // Mark as inactive
        *active = false;

        if let Some(handle) = &self.handle {
            handle.thread().unpark();
        }

        // Clear the current line and move to the next line
        let mut stdout = io::stdout();
        let pos = *self.initial_position.lock().unwrap();

        execute!(
            stdout,
            MoveTo(pos.0, pos.1),
            Clear(ClearType::CurrentLine),
            Print("\n")
        )
        .unwrap();
        stdout.flush().unwrap();

        // Show the cursor again
        let _ = execute!(io::stdout(), Show);
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop();
        // Ensure the spinner thread is terminated
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
