use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use std::{io, time::Duration};
use std::process::Command;

#[derive(Debug, PartialEq)]
enum InputMode {
    Normal,
    EditingSource,
    EditingRegex,
    EditingReplace,
}

struct App {
    source_text: String,
    regex_input: String,
    replace_input: String,
    output_text: String,
    input_mode: InputMode,
    status_message: String,
}

impl Default for App {
    fn default() -> App {
        App {
            source_text: "Praliné saber no ocupa el lugar de argentino.".to_string(),
            regex_input: String::new(),
            replace_input: String::new(),
            output_text: String::new(),
            input_mode: InputMode::Normal,
            status_message: "Listo. 's': Fuente, 'r': Regex, 't': Reemplazar, 'TAB': IA".to_string(),
        }
    }
}

impl App {
    fn apply_transform(&mut self) {
        if self.regex_input.is_empty() {
            self.output_text = self.source_text.clone();
            return;
        }

        let re = match regex::Regex::new(&self.regex_input) {
            Ok(r) => r,
            Err(e) => {
                self.output_text = format!("Regex Error: {}", e);
                return;
            }
        };

        if self.replace_input.is_empty() {
            // MODO FILTRO (Grep): Mostrar solo coincidencias
            let matches: Vec<&str> = re.find_iter(&self.source_text).map(|m| m.as_str()).collect();
            if matches.is_empty() {
                self.output_text = "(No hay coincidencias)".to_string();
            } else {
                self.output_text = matches.join(" | ");
            }
        } else {
            // MODO REEMPLAZO (Sed): Mostrar texto completo con cambios
            self.output_text = re.replace_all(&self.source_text, &self.replace_input).to_string();
        }
    }

    fn suggest_ai(&mut self) {
        self.status_message = "Consultando a Gemini IA...".to_string();
        
        let prompt = format!(
            "Give me ONLY the regex pattern (no text, no backticks, no markdown) to match or extract this: '{}' in the text: '{}'.",
            self.regex_input, self.source_text
        );

        let output = Command::new("cmd")
            .arg("/C")
            .arg("gemini")
            .arg("-p")
            .arg(prompt)
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let suggestion = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !suggestion.is_empty() {
                    let clean = suggestion
                        .replace("```regex", "")
                        .replace("```", "")
                        .replace("`", "")
                        .trim()
                        .to_string();
                    self.regex_input = clean;
                    self.status_message = "Sugerencia aplicada!".to_string();
                    self.apply_transform();
                } else {
                    self.status_message = "Gemini devolvió vacío.".to_string();
                }
            }
            Err(e) => {
                self.status_message = format!("Error de ejecución: {}", e);
            }
            Ok(out) => {
                let err_msg = String::from_utf8_lossy(&out.stderr);
                self.status_message = format!("Gemini Error: {}", err_msg.chars().take(30).collect::<String>());
            }
        }
    }
}

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::default();
    app.apply_transform(); 
    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {}", err);
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app)).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match app.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('s') => {
                            app.input_mode = InputMode::EditingSource;
                            app.source_text.clear();
                        }
                        KeyCode::Char('r') => {
                            app.input_mode = InputMode::EditingRegex;
                            app.regex_input.clear();
                        }
                        KeyCode::Char('t') => {
                            app.input_mode = InputMode::EditingReplace;
                            app.replace_input.clear();
                        }
                        KeyCode::Tab => {
                            app.suggest_ai();
                        }
                        _ => {}
                    },
                    InputMode::EditingSource => match key.code {
                        KeyCode::Esc => app.input_mode = InputMode::Normal,
                        KeyCode::Char(c) => app.source_text.push(c),
                        KeyCode::Backspace => { app.source_text.pop(); },
                        KeyCode::Enter => app.source_text.push('\n'),
                        _ => {}
                    },
                    InputMode::EditingRegex => match key.code {
                        KeyCode::Esc => app.input_mode = InputMode::Normal,
                        KeyCode::Char(c) => app.regex_input.push(c),
                        KeyCode::Backspace => { app.regex_input.pop(); },
                        KeyCode::Enter => app.input_mode = InputMode::Normal,
                        _ => {}
                    },
                    InputMode::EditingReplace => match key.code {
                        KeyCode::Esc => app.input_mode = InputMode::Normal,
                        KeyCode::Char(c) => app.replace_input.push(c),
                        KeyCode::Backspace => { app.replace_input.pop(); },
                        KeyCode::Enter => app.input_mode = InputMode::Normal,
                        _ => {}
                    },
                }
                app.apply_transform();
            }
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(3), // Title
                Constraint::Min(4),    // Source
                Constraint::Length(3), // Regex
                Constraint::Length(3), // Replace
                Constraint::Min(4),    // Output
                Constraint::Length(3), // Help
            ]
            .as_ref(),
        )
        .split(area);

    let mode_name = match app.input_mode {
        InputMode::Normal => "EXPLORAR",
        InputMode::EditingSource => "EDITANDO FUENTE",
        InputMode::EditingRegex => "EDITANDO REGEX",
        InputMode::EditingReplace => "EDITANDO REEMPLAZO",
    };

    let title = Paragraph::new(format!(" REGEX WYSIWYG - MODO: {} ", mode_name))
        .style(Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let source_style = if app.input_mode == InputMode::EditingSource { Style::default().fg(Color::Yellow) } else { Style::default() };
    f.render_widget(
        Paragraph::new(app.source_text.as_str())
            .style(source_style)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL).title(" [Source Text] ('s') ")),
        chunks[1]
    );

    let regex_style = if app.input_mode == InputMode::EditingRegex { Style::default().fg(Color::Magenta) } else { Style::default() };
    f.render_widget(
        Paragraph::new(app.regex_input.as_str())
            .style(regex_style)
            .block(Block::default().borders(Borders::ALL).title(" [Regex Pattern] ('r') ")),
        chunks[2]
    );

    let replace_style = if app.input_mode == InputMode::EditingReplace { Style::default().fg(Color::LightBlue) } else { Style::default() };
    f.render_widget(
        Paragraph::new(app.replace_input.as_str())
            .style(replace_style)
            .block(Block::default().borders(Borders::ALL).title(" [Replace With] ('t' - sed mode) ")),
        chunks[3]
    );

    f.render_widget(
        Paragraph::new(app.output_text.as_str())
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::Green))
            .block(Block::default().borders(Borders::ALL).title(" [Output Preview] ")),
        chunks[4]
    );

    let help_text = match app.input_mode {
        InputMode::Normal => format!("{} | q: Salir", app.status_message),
        _ => "Esc: Confirmar edición".to_string(),
    };
    f.render_widget(
        Paragraph::new(help_text)
            .style(Style::default().fg(Color::Gray))
            .block(Block::default().borders(Borders::ALL)),
        chunks[5]
    );
}
