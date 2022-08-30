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

use crate::config::*;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Copy, Clone)]
pub enum PromptType {
    UnsavedQuit,
    Find,
    Help,
    GoToLine,
}

impl PromptType {
    fn text(&self) -> &'static str {
        match self {
            Self::UnsavedQuit => "Unsaved changes, quit? (y/n)",
            Self::Find => "Search",
            Self::Help => "Help!",
            Self::GoToLine => "Go to",
        }
    }

    fn description(&self) -> String {
        match self {
            Self::Help => format!("Fox editor\nVersion {}\nConfig: {}\n\nCommands:\n ctrl-h: help\n ctrl-s: save\n ctrl-q: quit\n ctrl-f: search",
                            VERSION,
                            config_location().map(|p| p.display().to_string()).unwrap_or(String::from("unavailable"))
                          ),
            _ => String::new(),
        }
    }
}

#[derive(Clone)]
pub struct Prompt {
    pub prompt: PromptType,
    pub buf: String,
}

pub struct Fox {
    path_expanded: String,
    path: String,
    text: Vec<String>,
    cursor: (u16, u16),
    highlight: (u16, u16),
    scroll: u16,

    dirty: bool,
    prompt: Option<Prompt>,
    popup: Option<Prompt>,
    status: String,

    syntax: SyntaxReference,
    theme: Theme,

    bg: Color,
    fg: Color,
    gutter_bg: Color,
    gutter_fg: Color,
    highlight_bg: Color,
    highlight_fg: Color,
    header_bg: Color,
}

impl Fox {
    pub fn new(filename: &str) -> Result<Self> {
        let config = config();
        let default_config = Config::default();

        stdout().execute(EnterAlternateScreen)?;
        // stdout().execute(cursor::SetCursorShape(cursor::CursorShape::Line))?;
        enable_raw_mode()?;

        let filename_expanded = shellexpand::full(filename).map(|s| s.to_string()).unwrap_or(filename.to_string());
        let path = Path::new(&filename_expanded);
        let text: Vec<String> = if path.exists() { // Perhaps try_exists is better here
            std::fs::read_to_string(path).expect("File exists but cannot be opened for unknown reasons!").lines().map(|l| l.to_string()).collect()
        } else {
            vec![String::new()]
        };

        let ps = &carbon_dump::SYNTAX_SET;
        let ts = &carbon_dump::THEME_SET;
        let syntax = if let Some(extension) = &path.extension().map(|s| s.to_str().expect("Unparsable extension!")) {
            ps.find_syntax_by_extension(&extension).unwrap_or_else(|| ps.find_syntax_plain_text())
        } else if let Some(filename) = &path.file_name().map(|s| s.to_str().expect("Unparsable filename!")) {
            ps.find_syntax_by_extension(&filename).unwrap_or_else(|| ps.find_syntax_plain_text())
        } else {
            ps.find_syntax_plain_text()
        };
        let theme = ts.themes.get(&config.theme.name).unwrap_or_else(|| &ts.themes[&default_config.theme.name]); // gruvbox-dark
        let theme_is_dark = !config.theme.light_fix;

        let bg = theme.settings.background.unwrap_or(Color::BLACK);
        let fg = theme.settings.foreground.unwrap_or(Color::WHITE);
        let gutter_bg_mul = if theme_is_dark { 4.0 } else { 2.0 };
        let gutter_bg = theme.settings.gutter.unwrap_or(Color {
            r: (bg.r as f32 / 3.0 * gutter_bg_mul) as u8,
            g: (bg.g as f32 / 3.0 * gutter_bg_mul) as u8,
            b: (bg.b as f32 / 3.0 * gutter_bg_mul) as u8,
            a: bg.a,
        });
        let gutter_fg = theme.settings.gutter_foreground.unwrap_or(fg);
        let highlight_bg_default = if theme_is_dark { 48 } else { 132 };
        let highlight_fg_default = if theme_is_dark { 160 } else { 48 };
        let highlight_bg = theme.settings.selection.unwrap_or(theme.settings.highlight.unwrap_or(theme.settings.line_highlight.unwrap_or(theme.settings.find_highlight.unwrap_or(Color {
            r: highlight_bg_default,
            g: highlight_bg_default,
            b: highlight_bg_default,
            a: bg.a,
        }))));
        let highlight_fg = theme.settings.selection_foreground.unwrap_or(Color {
            r: highlight_fg_default,
            g: highlight_fg_default,
            b: highlight_fg_default,
            a: fg.a,
        });
        let header_bg_mul = if theme_is_dark { 5.0 } else { 1.5 };
        let header_bg = Color {
            r: (bg.r as f32 / 3.0 * header_bg_mul) as u8,
            g: (bg.g as f32 / 3.0 * header_bg_mul) as u8,
            b: (bg.b as f32 / 3.0 * header_bg_mul) as u8,
            a: bg.a,
        };

        Ok(Self {
            path_expanded: filename_expanded,
            path: filename.to_string(),
            text: text,
            cursor: (0,0),
            highlight: (0,0),
            scroll: 0,

            dirty: false,
            prompt: None,
            popup: None,
            status: String::new(),

            syntax: syntax.clone(),
            theme: theme.clone(),

            bg: bg,
            fg: fg,
            gutter_bg: gutter_bg,
            gutter_fg: gutter_fg,
            highlight_bg: highlight_bg,
            highlight_fg: highlight_fg,
            header_bg: header_bg,
        })
    }

    pub fn redraw(&mut self) -> Result<()> {
        use std::io::Write;
        use owo_colors::OwoColorize;

        stdout().execute(cursor::Hide)?;

        let terminal_size = size()?;

        // Header
        stdout().execute(cursor::MoveTo(0,0))?;
        let mut filename = self.path[..self.path.len().min(terminal_size.0 as usize)].to_string();
        if self.dirty { filename.push('*'); }
        let offset = (terminal_size.0 as usize - filename.len()) / 2;
        for _ in 0..offset {
            print!("{}", " ".on_truecolor(self.header_bg.r,self.header_bg.g,self.header_bg.b));
        }
        stdout().execute(cursor::MoveTo(offset as u16,0))?;
        print!("{}", filename.truecolor(self.fg.r, self.fg.g, self.fg.b).on_truecolor(self.header_bg.r,self.header_bg.g,self.header_bg.b));
        for _ in 0..offset + filename.len() % 1 + 1 {
            print!("{}", " ".on_truecolor(self.header_bg.r,self.header_bg.g,self.header_bg.b));
        }

        // Content
        let mut h = HighlightLines::new(&self.syntax, &self.theme);
        fn num_digits(n: u64, b: u32) -> u32 {
            (n as f64).log(b as f64).ceil() as u32
        }
        let width = (num_digits((self.scroll as usize + terminal_size.1 as usize) as u64, 10) + 1) as usize;
        for i in 1..terminal_size.1-1 {
            let line_num = i as usize + self.scroll as usize;
            stdout().execute(cursor::MoveTo(0,i))?;
            if let Some(line) = self.text.get(line_num-1) {
                print!("{}", format!(" {: >width$} ", line_num, width=width).truecolor(self.gutter_fg.r, self.gutter_fg.g, self.gutter_fg.b).on_truecolor(self.gutter_bg.r, self.gutter_bg.g, self.gutter_bg.b));

                // let line = &line[..line.len().min(terminal_size.0 as usize - width - 2)];
                let ranges: Vec<(Style, &str)> = h.highlight(&line, &carbon_dump::SYNTAX_SET);
                let line = as_24_bit_terminal_escaped(&ranges[..], true);
                let line = line.replace('\t', &format!("{}", "--->".truecolor(self.gutter_bg.r, self.gutter_bg.g, self.gutter_bg.b)));
                print!("{}", line);

                //Finish line
                for _ in cursor::position()?.0 .. terminal_size.0 { print!("{}", " ".on_truecolor(self.bg.r, self.bg.g, self.bg.b)); }
            } else {
                print!("{}", format!(" {: >width$} ", line_num, width=width).truecolor(self.gutter_fg.r, self.gutter_fg.g, self.gutter_fg.b).on_truecolor(self.gutter_bg.r, self.gutter_bg.g, self.gutter_bg.b));
                print!("{}", "~".truecolor(self.gutter_fg.r, self.gutter_fg.g, self.gutter_fg.b).on_truecolor(self.bg.r, self.bg.g, self.bg.b));
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
                    print!("{}", text.truecolor(self.highlight_fg.r, self.highlight_fg.g, self.highlight_fg.b).on_truecolor(self.highlight_bg.r, self.highlight_bg.g, self.highlight_bg.b));
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
        for _ in 0..terminal_size.0 { print!("{}", " ".on_truecolor(self.header_bg.r,self.header_bg.g,self.header_bg.b)); }
        stdout().execute(cursor::MoveTo(0,terminal_size.1))?;

        // Status/prompt
        if let Some(prompt) = &self.prompt {
            print!("{}", format!("{}: ", prompt.prompt.text()).truecolor(self.fg.r, self.fg.g, self.fg.b).on_truecolor(self.header_bg.r,self.header_bg.g,self.header_bg.b));
            print!("{}", prompt.buf.truecolor(self.fg.r, self.fg.g, self.fg.b).on_truecolor(self.header_bg.r,self.header_bg.g,self.header_bg.b));
        } else {
            print!("{}", self.status.truecolor(self.fg.r, self.fg.g, self.fg.b).on_truecolor(self.header_bg.r,self.header_bg.g,self.header_bg.b));
            self.status = String::new();
        }

        // Cursor location
        let footer_loc = format!("{}:{}", self.cursor.0+1, self.cursor.1+1);
        stdout().execute(cursor::MoveTo(terminal_size.0-footer_loc.len() as u16,terminal_size.1))?;
        print!("{}", footer_loc.truecolor(self.fg.r, self.fg.g, self.fg.b).on_truecolor(self.header_bg.r,self.header_bg.g,self.header_bg.b));

        // Popup rendering
        if let Some(popup) = &self.popup {
            let (w,h) = terminal_size;
            let x = w / 6;
            let y = h / 6;
            let w = w / 6 * 4;
            let h = h / 6 * 4;
            for i in 0..h {
                stdout().execute(cursor::MoveTo(x,y+i))?;
                for _ in 0..w {
                    print!("{}", " ".on_truecolor(self.gutter_bg.r, self.gutter_bg.g, self.gutter_bg.b));
                }
            }

            let max_text_width = (w - 2) as usize;
            let title = popup.prompt.text();
            let len = title.len().min(max_text_width);
            let title = &title[..len];
            let offset = (max_text_width - len) / 2 - len % 2;
            stdout().execute(cursor::MoveTo(x+1+offset as u16,y+1))?;
            print!("{}", title.truecolor(self.fg.r, self.fg.g, self.fg.b).on_truecolor(self.gutter_bg.r, self.gutter_bg.g, self.gutter_bg.b));

            let desc = popup.prompt.description();
            let description: Vec<&str> = desc.lines().collect();
            for i in 0..description.len().min((h.max(3)-3) as usize) {
                let line = description[i];
                stdout().execute(cursor::MoveTo(x+1,y+3+i as u16))?;
                print!("{}", line.truecolor(self.fg.r, self.fg.g, self.fg.b).on_truecolor(self.gutter_bg.r, self.gutter_bg.g, self.gutter_bg.b));
            }
        }

        // Move cursor to show typing location
        let cpos_y = if self.scroll > self.cursor.1 { 0 } else { self.cursor.1 - self.scroll } + 1;
        if cpos_y < 1 || cpos_y >= terminal_size.1-1 {
            stdout().execute(cursor::Hide)?;
        } else {
            if self.highlight == self.cursor { stdout().execute(cursor::Show)?; } else { stdout().execute(cursor::Hide)?; }
            let tab_count = self.text[self.cursor.1 as usize][..self.cursor.0 as usize].matches("\t").count();
            let tab_offset = tab_count * 3;
            stdout().execute(cursor::MoveTo(self.cursor.0 + width as u16 + 2 + tab_offset as u16, cpos_y))?;
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

    pub fn popup(&mut self, popup: PromptType) {
        self.popup = Some(Prompt {
            prompt: popup,
            buf: String::new(),
        });
    }

    pub fn save(&mut self) -> Result<()> {
        std::fs::write(&self.path_expanded, self.text.join("\n"))?;
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
        if let Some(popup) = &mut self.popup {
            popup.buf.push(c);
        } else if let Some(prompt) = &mut self.prompt {
            prompt.buf.push(c);
        } else {
            self.dirty = true;
            if let Some(line) = self.text.get(self.cursor.1 as usize) {
                if self.cursor.0 == 0 {
                    let line = line.clone();
                    let mut result = String::from(c);
                    result.push_str(&line);
                    self.text[self.cursor.1 as usize] = result;
                    self.cursor_horizontal(1);
                } else {
                    let line = line.clone();
                    let (left, right) = line.split_at(self.cursor.0 as usize);
                    let mut result = String::from(left);
                    result.push(c);
                    result.push_str(right);
                    self.text[self.cursor.1 as usize] = result;
                    self.cursor_horizontal(1);
                }
            }
        }
    }

    pub fn pop_char(&mut self) {
        if let Some(popup) = &mut self.popup {
            popup.buf.pop();
        } else if let Some(prompt) = &mut self.prompt {
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
                        self.cursor.1 != 0
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
        if self.prompt.is_none() && self.popup.is_none() {
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
            self.cursor_start_of_line();
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

    pub fn go_to_line(&mut self, line: u16) {
        let i = line.min(self.text.len() as u16 - 1);
        self.cursor.1 = i;
        self.highlight.1 = self.cursor.1;
        self.cursor_start_of_line();
        // Scrolling
        let (_, height) = size().expect("Failed to query terminal size!");
        let offset = self.cursor.1 as i16 - self.scroll as i16;
        if offset >= height as i16 - 2 {
            self.scroll += offset as u16;
        } else if offset < 0 {
            self.scroll -= offset.abs() as u16;
        }
    }

    pub fn swap_down(&mut self) {
        if let Some(line_down) = self.text.get(self.cursor.1 as usize + 1) {
            let line = self.text.get(self.cursor.1 as usize).expect("How did we get here?").clone();
            self.text[self.cursor.1 as usize] = line_down.clone();
            self.text[self.cursor.1 as usize + 1] = line.to_string();
            self.cursor_vertical(1);
            self.dirty = true;
        }
    }

    pub fn swap_up(&mut self) {
        self.cursor_vertical(-1);
        self.swap_down();
        self.cursor_vertical(-1);
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
                        KeyCode::Char('h') => editor.popup(PromptType::Help),
                        KeyCode::Char('k') => editor.prompt(PromptType::GoToLine),
                        KeyCode::Char('v') => {
                            if let Ok(clipboard) = terminal_clipboard::get_string() {
                                for line in clipboard.lines() {
                                    for c in line.chars() {
                                        editor.push_char(c);
                                    }
                                    editor.enter();
                                    editor.cursor_start_of_line();
                                }
                            }
                        }

                        KeyCode::Down => editor.swap_down(),
                        KeyCode::Up => editor.swap_up(),

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
                            fn handle_prompt(editor: &mut Fox, prompt: Prompt, is_popup: bool) -> bool {
                                let ans = &prompt.buf;
                                if match prompt.prompt {
                                    PromptType::UnsavedQuit => { if ans == "y" || ans == "ye" || ans == "yes" { return true; }; true },
                                    PromptType::Find => {
                                        let found = editor.find_next(ans);
                                        if !found {
                                            editor.status = String::from("Could not find string!");
                                        }
                                        !found
                                    },
                                    PromptType::Help => true,
                                    PromptType::GoToLine => {
                                        if let Ok(num) = ans.parse::<u16>() {
                                            editor.go_to_line(num.max(1) - 1);
                                        }
                                        true
                                    }
                                } {
                                    if is_popup {
                                        editor.popup = None;
                                    } else {
                                        editor.prompt = None;
                                    }
                                }
                                false
                            }
                            if editor.popup.is_some() {
                                let popup = editor.popup.as_ref().unwrap().clone();
                                if handle_prompt(&mut editor, popup, true) { break 'app; }
                            } else if editor.prompt.is_some() {
                                let prompt = editor.prompt.as_ref().unwrap().clone();
                                if handle_prompt(&mut editor, prompt, false) { break 'app; }
                            } else {
                                editor.enter();
                                editor.dirty = true;
                            }
                        },
                        KeyCode::Esc => if editor.popup.is_some() { editor.popup = None; } else if editor.prompt.is_some() { editor.prompt = None; }

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