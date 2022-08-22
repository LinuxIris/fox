use std::io::{stdout};
use std::path::Path;

use crossterm::{
    terminal::*,
    cursor,
    ExecutableCommand, Result,
    event::*,
};

pub struct Fox {
    path: String,
    text: Vec<String>,
    cursor: (u16, u16),
    highlight: (u16, u16),
    scroll: u16,
}

impl Fox {
    pub fn new(filename: &str) -> Result<Self> {
        stdout().execute(EnterAlternateScreen)?;
        // stdout().execute(cursor::SetCursorShape(cursor::CursorShape::Line))?;
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
            highlight: (0,0),
            scroll: 0,
        })
    }

    pub fn redraw(&self) -> Result<()> {
        use std::io::Write;
        use owo_colors::OwoColorize;

        let terminal_size = size()?;
        // stdout().execute(Clear(ClearType::All))?;

        // Header
        stdout().execute(cursor::MoveTo(0,0))?;
        let filename = &self.path[..self.path.len().min(terminal_size.0 as usize)];
        let offset = (terminal_size.0 as usize - filename.len()) / 2;
        for _ in 0..offset {
            print!("{}", " ".on_truecolor(64,64,64));
        }
        stdout().execute(cursor::MoveTo(offset as u16,0))?;
        print!("{}", filename.on_truecolor(64,64,64));
        for _ in 0..offset + filename.len() % 1 + 1 {
            print!("{}", " ".on_truecolor(64,64,64));
        }

        // Content
        let width = ((self.scroll as usize + terminal_size.1 as usize).checked_log10().unwrap_or(0) + 1) as usize;
        for i in 1..terminal_size.1-1 {
            let line_num = i as usize + self.scroll as usize;
            stdout().execute(cursor::MoveTo(0,i))?;
            if let Some(line) = self.text.get(line_num-1) {
                // print!("{: >2} {}", line_num, line);
                print!("{}", format!(" {: >width$} ", line_num, width=width).on_truecolor(48,48,48));
                print!("{}", &line[..line.len().min(terminal_size.0 as usize - width - 2)].replace('\t', "  "));
                //Finish line
                for _ in cursor::position()?.1 .. terminal_size.1 { print!(" "); }
            } else {
                print!("{}", format!(" {: >width$} ", line_num, width=width).on_truecolor(48,48,48));
                print!("~");
                //Finish line
                for _ in cursor::position()?.1 .. terminal_size.1 { print!(" "); }
            }
        }

        // Highlight
        if self.highlight != self.cursor {
            // let hpos_y = if self.scroll > self.highlight.1 { 0 } else { self.highlight.1 - self.scroll } + 1;
            // let cpos_y = if self.scroll > self.cursor.1 { 0 } else { self.cursor.1 - self.scroll } + 1;
            if self.highlight.1 == self.cursor.1 {
                // Single line selection
                if let Some(line) = self.text.get(self.cursor.1 as usize) {
                    let min_x = self.highlight.0.min(self.cursor.0) as usize;
                    let max_x = self.highlight.0.max(self.cursor.0) as usize;
                    let text = &line[min_x..max_x];
                    let cpos_y = if self.scroll > self.cursor.1 { 0 } else { self.cursor.1 - self.scroll } + 1;
                    stdout().execute(cursor::MoveTo((min_x+width+2) as u16, cpos_y))?;
                    print!("{}", text.on_truecolor(160,160,160));
                }
            } else {
                // Multi line selection
                todo!();
                // let min_y = self.highlight.1.min(self.cursor.1);
                // let max_y = self.highlight.1.max(self.cursor.1);
                // for i in min_y..max_y {
                //
                // }
            }
        }

        // Footer
        stdout().execute(cursor::MoveTo(0,terminal_size.1))?;
        for _ in 0..terminal_size.0 { print!("{}", " ".on_truecolor(64,64,64)); }
        stdout().execute(cursor::MoveTo(0,terminal_size.1))?;
        print!("{}", "status here".on_truecolor(64,64,64));
        let footer_loc = format!("{}:{}", self.cursor.0+1, self.cursor.1+1);
        stdout().execute(cursor::MoveTo(terminal_size.0-footer_loc.len() as u16,terminal_size.1))?;
        print!("{}", footer_loc.on_truecolor(64,64,64));

        // Move cursor to show typing location
        let cpos_y = if self.scroll > self.cursor.1 { 0 } else { self.cursor.1 - self.scroll } + 1;
        if cpos_y < 1 || cpos_y >= terminal_size.1-1 {
            stdout().execute(cursor::Hide)?;
        } else {
            if self.highlight == self.cursor { stdout().execute(cursor::Show)?; } else { stdout().execute(cursor::Hide)?; }
            stdout().execute(cursor::MoveTo(self.cursor.0 + width as u16 + 2, cpos_y))?;
        }

        stdout().flush()?;
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        std::fs::write(&self.path, self.text.join("\n"))?;
        Ok(())
    }

    pub fn push_char(&mut self, c: char) {
        if let Some(line) = self.text.get(self.cursor.1 as usize) {
            let line = line.clone();
            let (left, right) = line.split_at(self.cursor.0 as usize);
            let mut result = String::from(left);
            result.push(c);
            result.push_str(right);
            self.text[self.cursor.1 as usize] = result;
            match c {
                '\t' => self.cursor.0 += 2,
                _ => self.cursor.0 += 1,
            }
        }
    }

    pub fn pop_char(&mut self) {
        if let Some(line) = self.text.get(self.cursor.1 as usize) {
            if self.cursor.0 == 0 {
                return;
            }
            let line = line.clone();
            let (left, right) = line.split_at(self.cursor.0 as usize);
            let mut result = String::from(left);
            result.pop();
            result.push_str(right);
            self.text[self.cursor.1 as usize] = result;
            self.cursor.0 -= 1;
        }
    }

    pub fn pop_char_del(&mut self) {
        if let Some(line) = self.text.get(self.cursor.1 as usize) {
            if self.cursor.0 as usize >= line.len() {
                return;
            }
            let line = line.clone();
            let (left, right) = line.split_at(self.cursor.0 as usize);
            let mut result = String::from(left);
            result.push_str(&right[1..]);
            self.text[self.cursor.1 as usize] = result;
        }
    }

    pub fn enter(&mut self) {
        if let Some(line) = self.text.get(self.cursor.1 as usize) {
            if self.cursor.0 as usize >= line.len() {
                self.text.push(String::new());
                self.cursor_vertical(1);
                self.cursor.0 = 0;
                return;
            }
            let (left, right) = line.split_at(self.cursor.0 as usize);
            let right = String::from(right);
            self.text[self.cursor.1 as usize] = String::from(left);
            self.text.insert(self.cursor.1 as usize + 1, right);
            self.cursor_vertical(1);
            self.cursor.0 = 0;
        }
    }

    pub fn cursor_vertical(&mut self, i: i16) {
        let old = self.cursor.1;
        if i > 0 {
            self.cursor.1 += i as u16;
        } else if self.cursor.1 > 0 {
            self.cursor.1 -= i.abs() as u16;
        }
        if let Some(line) = self.text.get(self.cursor.1 as usize) {
            if self.cursor.0 > line.len() as u16 {
                self.cursor.0 = line.len() as u16;
            }
        } else {
            self.cursor.1 = old;
        }
        self.highlight = self.cursor;
    }

    //TODO: Perhaps move the cursor to the next/previous line if at the end/start of the current line?
    pub fn cursor_horizontal(&mut self, i: i16) {
        if self.highlight != self.cursor {
            let (start, end) = {
                if self.highlight.1 == self.cursor.1 {
                    // Single line selection
                    let min_x = self.highlight.0.min(self.cursor.0);
                    let max_x = self.highlight.0.max(self.cursor.0);
                    ((min_x,self.cursor.1),(max_x,self.cursor.1))
                } else {
                    // Multi line selection
                    todo!();
                }
            };
            if i > 0 {
                self.cursor = end;
            } else {
                self.cursor = start;
            }
        } else {
            let old = self.cursor.0;
            if i > 0 {
                self.cursor.0 += i as u16;
            } else if self.cursor.0 > 0 {
                self.cursor.0 -= i.abs() as u16;
            }
            if self.cursor.0 as usize > self.text[self.cursor.1 as usize].len() {
                self.cursor.0 = old;
            }
        }
        self.highlight = self.cursor;
    }

    pub fn highlight_horizontal(&mut self, i: i16) {
        let old = self.highlight.0;
        if i > 0 {
            self.highlight.0 += i as u16;
        } else if self.highlight.0 > 0 {
            self.highlight.0 -= i.abs() as u16;
        }
        if self.highlight.0 as usize > self.text[self.highlight.1 as usize].len() {
            self.highlight.0 = old;
        }
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
    'app: loop {
        match read()? {
            Event::Key(key) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    match key.code {
                        KeyCode::Char('q') => break 'app,
                        KeyCode::Char('s') => editor.save()?, //TODO: If also holding shift, save as?
                        _ => {},
                    }
                } else if key.modifiers.contains(KeyModifiers::SHIFT) {
                    match key.code {
                        KeyCode::Char(c) => c.to_uppercase().for_each(|c| editor.push_char(c)),
                        KeyCode::Left => editor.highlight_horizontal(-1),
                        KeyCode::Right => editor.highlight_horizontal(1),
                        _ => {},
                    }
                } else {
                    match key.code {
                        KeyCode::Char(c) => editor.push_char(c),
                        KeyCode::Tab => editor.push_char('\t'),
                        KeyCode::Backspace => editor.pop_char(),
                        KeyCode::Delete => editor.pop_char_del(),
                        KeyCode::Up => editor.cursor_vertical(-1),
                        KeyCode::Down => editor.cursor_vertical(1),
                        KeyCode::Right => editor.cursor_horizontal(1),
                        KeyCode::Left => editor.cursor_horizontal(-1),
                        KeyCode::Enter => editor.enter(),
                        _ => {},
                    }
                }
            },
            _ => {},
        }
        editor.redraw()?;
    }
    Ok(())
}
