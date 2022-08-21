use std::io::{stdout};
use std::path::Path;

use crossterm::{
    style::*,
    terminal::*,
    cursor,
    ExecutableCommand, Result,
    event,
};

pub struct Fox {
    path: String,
    text: Vec<String>,
    cursor: (u16, u16),
}

impl Fox {
    pub fn new(filename: &str) -> Result<Self> {
        stdout().execute(EnterAlternateScreen)?;
        enable_raw_mode()?;

        let path = Path::new(filename);
        let text: Vec<String> = if path.exists() { // Perhaps try_exists is better here
            std::fs::read_to_string(path).expect("File exists but cannot be opened for unknown reasons!").lines().map(|l| l.to_string()).collect()
        } else {
            vec![String::new()]
        };

        Ok(Self {
            path: filename.to_string(),
            text: text,
            cursor: (0,0),
        })
    }

    pub fn redraw(&self) -> Result<()> {
        let terminal_size = size();
        stdout().execute(Clear(ClearType::All))?;
        stdout().execute(cursor::MoveTo(0,0))?;
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        std::fs::write(&self.path, self.text.join("\n"))?;
        Ok(())
    }
}

impl Drop for Fox {
    fn drop(&mut self) {
        let _ = stdout().execute(LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

pub fn run(filename: &str) -> Result<()> {
    let mut editor = Fox::new(filename)?;

    Ok(())
}
