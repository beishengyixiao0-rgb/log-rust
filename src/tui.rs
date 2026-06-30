use ratatui::{backend::CrosstermBackend, Terminal};

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use std::io::{stdout, Write};

pub fn start_tui() {
    enable_raw_mode().unwrap();

    let mut stdout = stdout();

    execute!(stdout, EnterAlternateScreen).unwrap();

    let backend = CrosstermBackend::new(stdout);

    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            let size = f.size();

            let text = format!("BitLog TUI\nQPS: {:.2}", crate::stats::GLOBAL_STATS.qps());

            let paragraph = ratatui::widgets::Paragraph::new(text);

            f.render_widget(paragraph, size);
        })
        .unwrap();

    disable_raw_mode().unwrap();
    let stdout = terminal.backend_mut();
    execute!(stdout, LeaveAlternateScreen).unwrap();
    let _ = stdout.flush();
}
