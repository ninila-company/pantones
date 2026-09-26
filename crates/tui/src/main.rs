mod app;

use std::io;
use std::time::Duration;

use app::App;
use clap::Parser;
use crossterm::event;

#[derive(Parser)]
#[command(name = "pantones", version, about = "Pantone warehouse stock tracker")]
struct Cli {
    /// Path to the CSV file
    #[arg(default_value = "pantones.csv")]
    file: String,
}

fn main() {
    let cli = Cli::parse();

    let mut app = match App::new(cli.file) {
        Ok(app) => app,
        Err(e) => {
            eprintln!("Failed to start: {e}");
            std::process::exit(1);
        }
    };

    let result: Result<(), io::Error> = ratatui::run(|terminal| {
        loop {
            terminal.draw(|frame| app.draw(frame))?;
            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    event::Event::Key(key) => {
                        if app.handle_event(event::Event::Key(key))? {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            app.tick();
        }
        Ok(())
    });

    match result {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
        Err(e) => {
            eprintln!("TUI error: {e}");
            std::process::exit(1);
        }
    }
}
