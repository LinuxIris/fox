use std::io::{stdout};
use std::path::Path;

use crossterm::{
    terminal::*,
    cursor,
    ExecutableCommand, Result,
    event::*,
};

use syntect::{
    easy::HighlightLines,
    highlighting::{Style, Theme, Color},
    util::as_24_bit_terminal_escaped,
    parsing::SyntaxReference,
};

#[derive(Copy, Clone)]
pub enum PromptType {
    UnsavedQuit,
    Find,
}

impl PromptType {
    fn text(&self) -> &'static str {
        match self {
            Self::UnsavedQuit => "Unsaved changes, quit? (y/n)",
            Self::Find => "Search",
        }
    }
}

#[derive(Clone)]
pub struct Prompt {
    pub prompt: PromptType,
    pub buf: String,
}

pub struct Fox {
    path: String,
    text: Vec<String>,
    cursor: (u16, u16),
    highlight: (u16, u16),
    scroll: u16,

    dirty: bool,
    prompt: Option<Prompt>,
    status: String,

    syntax: SyntaxReference,
    theme: Theme,

    bg: Color,
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

        let ps = &carbon_dump::SYNTAX_SET;
        let ts = &carbon_dump::THEME_SET;
        let syntax = if let Some(extension) = &path.extension().map(|s| s.to_str().expect("Unparsable extension!")) {
            ps.find_syntax_by_extension(&extension).unwrap()
        } else {
            ps.find_syntax_plain_text()
        };
        let theme = &ts.themes["gruvbox-dark"];

        let bg = theme.settings.background.unwrap_or(Color::BLACK);

        Ok(Self {
            path: filename.to_string(),
            text: text,
            cursor: (0,0),
            highlight: (0,0),
            scroll: 0,

            dirty: false,
            prompt: None,
            status: String::new(),

            syntax: syntax.clone(),
            theme: theme.clone(),

            bg: bg,
        })
    }

    pub fn redraw(&mut self) -> Result<()> {
        use std::io::Write;
        use owo_colors::OwoColorize;

        let terminal_size = size()?;
        // stdout().execute(Clear(ClearType::All))?;

        // Header
        stdout().execute(cursor::MoveTo(0,0))?;
        let mut filename = self.path[..self.path.len().min(terminal_size.0 as usize)].to_string();
        if self.dirty { filename.push('*'); }
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
        let mut h = HighlightLines::new(&self.syntax, &self.theme);
        let width = ((self.scroll as usize + terminal_size.1 as usize).checked_log10().unwrap_or(0) + 1) as usize;
        for i in 1..terminal_size.1-1 {
            let line_num = i as usize + self.scroll as usize;
            stdout().execute(cursor::MoveTo(0,i))?;
            if let Some(line) = self.text.get(line_num-1) {
                print!("{}", format!(" {: >width$} ", line_num, width=width).on_truecolor(48,48,48));

                let line = line.replace('\t', "  ");
                // let line = &line[..line.len().min(terminal_size.0 as usize - width - 2)];
                let ranges: Vec<(Style, &str)> = h.highlight(&line, &carbon_dump::SYNTAX_SET);
                print!("{}", as_24_bit_terminal_escaped(&ranges[..], true));

                //Finish line
                for _ in cursor::position()?.0 .. terminal_size.0 { print!("{}", " ".on_truecolor(self.bg.r, self.bg.g, self.bg.b)); }
            } else {
                print!("{}", format!(" {: >width$} ", line_num, width=width).on_truecolor(self.bg.r, self.bg.g, self.bg.b));
                print!("{}", "~".on_truecolor(self.bg.r, self.bg.g, self.bg.b));
                //Finish line
                for _ in cursor::position()?.0 .. terminal_size.0 { print!("{}", " ".on_truecolor(self.bg.r, self.bg.g, self.bg.b)); }
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
                    print!("{}", text.truecolor(32,32,32).on_truecolor(160,160,160));
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

        // Status/prompt
        if let Some(prompt) = &self.prompt {
            print!("{}", format!("{}: ", prompt.prompt.text()).on_truecolor(64,64,64));
            print!("{}", prompt.buf.on_truecolor(64,64,64));
        } else {
            print!("{}", self.status.on_truecolor(64,64,64));
            self.status = String::new();
        }

        // Cursor location
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

    pub fn prompt(&mut self, prompt: PromptType) {
        self.prompt = Some(Prompt {
            prompt: prompt,
            buf: String::new(),
        });
    }

    pub fn save(&mut self) -> Result<()> {
        std::fs::write(&self.path, self.text.join("\n"))?;
        self.dirty = false;
        self.status = String::from("Saved!");
        Ok(())
    }

    fn find_from(&mut self, s: &str, y: usize) -> bool {
        for i in y..self.text.len() {
            if let Some(line) = self.text.get(i) {
                let mut line = line.to_string();
                if i == self.cursor.1 as usize {
                    line = line[self.cursor.0 as usize..].to_string();
                }
                if let Some(x) = line.find(s) {
                    self.highlight.0 = x as u16;
                    self.cursor.0 = self.highlight.0 + s.len() as u16;
                    self.cursor.1 = i as u16;
                    self.highlight.1 = self.cursor.1;

                    // Calculate scroll - little bit fucked up rn lol
                    let (_, mut height) = size().expect("Failed to query terminal size!");
                    height -= 3;
                    if self.cursor.1 as i16 - self.scroll as i16 > height as i16 {
                        self.scroll += ((self.cursor.1 as i16 - self.scroll as i16) - height as i16) as u16;
                    } else if (self.cursor.1 as i16 - self.scroll as i16) < 0 {
                        self.scroll = 0;
                        if self.cursor.1 as i16 - self.scroll as i16 > height as i16 {
                            self.scroll += ((self.cursor.1 as i16 - self.scroll as i16) - height as i16) as u16;
                        }
                    }

                    return true;
                }
            }
        }
        false
    }

    pub fn find_next(&mut self, s: &str) -> bool {
        if !self.find_from(s, self.cursor.1 as usize) {
            if !self.find_from(s, 0) {
                return false;
            }
        }
        true
    }

    pub fn push_char(&mut self, c: char) {
        if let Some(prompt) = &mut self.prompt {
            prompt.buf.push(c);
        } else {
            self.dirty = true;
            if let Some(line) = self.text.get(self.cursor.1 as usize) {
                let line = line.clone();
                let (left, right) = line.split_at(self.cursor.0 as usize);
                let mut result = String::from(left);
                result.push(c);
                result.push_str(right);
                self.text[self.cursor.1 as usize] = result;
                self.cursor_horizontal(match c {
                    '\t' => 2,
                    _ => 1,
                });
            }
        }
    }

    pub fn pop_char(&mut self) {
        if let Some(prompt) = &mut self.prompt {
            prompt.buf.pop();
        } else {
            self.dirty = true;
            if self.highlight != self.cursor {
                if self.highlight.1 == self.cursor.1 {
                    // Single line selection
                    let min_x = self.highlight.0.min(self.cursor.0);
                    let max_x = self.highlight.0.max(self.cursor.0);
                    let pop_count = max_x - min_x;
                    if let Some(line) = self.text.get(self.cursor.1 as usize) {
                        let line = line.clone();
                        let (left, right) = line.split_at(max_x as usize);
                        let mut result = String::from(left);
                        for _ in 0..pop_count { result.pop(); }
                        result.push_str(right);
                        self.text[self.cursor.1 as usize] = result;
                        self.cursor_horizontal(-(pop_count as i16));
                    }
                } else {
                    // Multi line selection
                    todo!();
                }
            } else {
                let remove = if let Some(line) = self.text.get(self.cursor.1 as usize) {
                    if self.cursor.0 == 0 {
                        true
                    } else {
                        let line = line.clone();
                        let (left, right) = line.split_at(self.cursor.0 as usize);
                        let mut result = String::from(left);
                        result.pop();
                        result.push_str(right);
                        self.text[self.cursor.1 as usize] = result;
                        self.cursor_horizontal(-1);
                        false
                    }
                } else {
                    false
                };

                if remove {
                    let cur = self.text.get(self.cursor.1 as usize).unwrap().clone();
                    self.text.remove(self.cursor.1 as usize);
                    self.cursor_vertical(-1);
                    self.cursor_end_of_line();
                    if let Some(line) = self.text.get_mut(self.cursor.1 as usize) {
                        line.push_str(&cur);
                    }
                }
            }
        }
    }

    pub fn pop_char_del(&mut self) {
        if self.prompt.is_none() {
            self.dirty = true;
            if self.highlight != self.cursor {
                self.pop_char();
            } else if let Some(line) = self.text.get(self.cursor.1 as usize) {
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
    }

    pub fn enter(&mut self) {
        if let Some(line) = self.text.get(self.cursor.1 as usize) {
            if self.cursor.0 as usize >= line.len() {
                self.text.insert(self.cursor.1 as usize + 1, String::new());
                self.cursor_vertical(1);
                self.cursor_start_of_line();
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

    pub fn cursor_start_of_line(&mut self) {
        self.cursor.0 = 0;
        self.highlight.0 = 0;
    }

    pub fn cursor_end_of_line(&mut self) {
        if let Some(line) = self.text.get(self.cursor.1 as usize) {
            self.cursor.0 = line.len() as u16;
            self.highlight.0 = self.cursor.0;
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

        // Scrolling
        let (_, height) = size().expect("Failed to query terminal size!");
        let offset = self.cursor.1 as i16 - self.scroll as i16;
        if offset >= height as i16 - 2 {
            self.scroll += 1;
        } else if offset < 0 {
            self.scroll -= 1;
        }
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
            let old_y = self.cursor.1;
            if i > 0 {
                self.cursor.0 += i as u16;
            } else if self.cursor.0 > 0 {
                self.cursor.0 -= i.abs() as u16;
            } else {
                // Start of the line and moving left
                self.cursor_vertical(-1);
                if self.cursor.1 != old_y { self.cursor_end_of_line(); }
            }
            if self.cursor.0 as usize > self.text[self.cursor.1 as usize].len() {
                self.cursor_vertical(1);
                if self.cursor.1 != old_y { self.cursor_start_of_line(); } else { self.cursor.0 = old; }
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
                        KeyCode::Char('q') => {
                            if editor.dirty {
                                editor.prompt(PromptType::UnsavedQuit);
                            } else {
                                break 'app;
                            }
                        },
                        KeyCode::Char('s') => editor.save()?, //TODO: If also holding shift, save as?
                        KeyCode::Char('f') => editor.prompt(PromptType::Find),
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
                        KeyCode::Enter => {
                            if editor.prompt.is_some() {
                                let prompt = editor.prompt.as_ref().unwrap().clone();
                                let ans = &prompt.buf;
                                if match prompt.prompt {
                                    PromptType::UnsavedQuit => { if ans == "y" || ans == "ye" || ans == "yes" { break 'app; }; true },
                                    PromptType::Find => {
                                        let found = editor.find_next(ans);
                                        if !found {
                                            editor.prompt = None;
                                            editor.status = String::from("Could not find string!");
                                        }
                                        false
                                    },
                                } {
                                    editor.prompt = None;
                                }
                            } else {
                                editor.enter();
                                editor.dirty = true;
                            }
                        },
                        KeyCode::Esc => if editor.prompt.is_some() { editor.prompt = None; }

                        KeyCode::Up => editor.cursor_vertical(-1),
                        KeyCode::Down => editor.cursor_vertical(1),
                        KeyCode::Right => editor.cursor_horizontal(1),
                        KeyCode::Left => editor.cursor_horizontal(-1),
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